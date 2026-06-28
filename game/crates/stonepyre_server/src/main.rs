mod config;
mod error;
mod http;
mod state;
mod game;

use axum::Router;
use sqlx::{Connection, PgPool, Row};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};

use crate::{config::Config, state::AppState};

const MARKET_SIM_LOCK_KEY: i64 = 9_007_199_254_740_993;
const GAME_SIM_LOCK_KEY: i64 = 9_007_199_254_740_994;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let log_level = std::env::var("SERVER_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    let cfg = Config::from_env()?;
    let tick_hz: u32 = std::env::var("GAME_TICK_HZ")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let snapshot_hz: u32 = std::env::var("GAME_SNAPSHOT_HZ")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(tick_hz); // default: broadcast every tick

    info!("stonepyre_server starting...");
    info!("bind={}", cfg.bind_addr);
    info!("db={}", cfg.database_url);

    let pool = sqlx::PgPool::connect(&cfg.database_url).await?;
    let game = game::GameRuntime::new(tick_hz);
    let state = AppState::new(cfg.clone(), pool.clone(), game.clone());

    let mut market_lock_conn = sqlx::PgConnection::connect(&cfg.database_url).await?;
    let market_got_lock: bool = sqlx::query("SELECT pg_try_advisory_lock($1) AS ok")
        .bind(MARKET_SIM_LOCK_KEY)
        .fetch_one(&mut market_lock_conn)
        .await?
        .try_get("ok")?;

    if market_got_lock {
        info!("market sim lock acquired; starting sim loop inside server");

        let market_cfg = stonepyre_market::config::MarketConfig::from_env()?;
        let market_state = stonepyre_market::state::AppState::new(market_cfg, pool.clone());

        let assets_dir = std::path::PathBuf::from(market_state.cfg.assets_dir.clone());

        match stonepyre_market::sim::company_factory::CompanyFactory::load(&market_state.pool, &assets_dir).await {
            Ok(factory) => {
                *market_state.company_factory.write().await = Some(factory);
                info!("CompanyFactory loaded from {}", assets_dir.display());
            }
            Err(e) => {
                warn!("CompanyFactory NOT loaded (IPO top-up disabled): {e:?}");
            }
        }

        stonepyre_market::sim::start_sim_loop(market_state);
    } else {
        warn!("market sim lock NOT acquired; another process is running the sim loop. server will NOT tick the market.");
    }

    let _keep_market_lock_conn_alive = market_lock_conn;

    let mut game_lock_conn = sqlx::PgConnection::connect(&cfg.database_url).await?;
    let game_got_lock: bool = sqlx::query("SELECT pg_try_advisory_lock($1) AS ok")
        .bind(GAME_SIM_LOCK_KEY)
        .fetch_one(&mut game_lock_conn)
        .await?
        .try_get("ok")?;

    if game_got_lock {
        info!(
            "game sim lock acquired; starting game loops tick_hz={} snapshot_hz={}",
            tick_hz, snapshot_hz
        );

        start_game_loops(game.clone(), pool.clone(), tick_hz, snapshot_hz);
    } else {
        warn!("game sim lock NOT acquired; another process is running the game loop. server will NOT tick the game.");
    }

    let _keep_game_lock_conn_alive = game_lock_conn;

    let app: Router = http::router(state);

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("stonepyre_server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    warn!("stonepyre_server stopped");
    Ok(())
}

fn start_game_loops(game: game::GameRuntime, db: PgPool, tick_hz: u32, snapshot_hz: u32) {
    tokio::spawn({
        let game = game.clone();
        let db = db.clone();
        async move {
            let dt = std::time::Duration::from_millis((1000 / tick_hz.max(1)) as u64);
            let mut interval = tokio::time::interval(dt);

            loop {
                interval.tick().await;
                let events = {
                    let mut sim = game.sim.write().await;
                    sim.step()
                };

                for event in events {
                    match event {
                        game::sim::GameSimEvent::Server(msg) => {
                            game.hub.broadcast(msg);
                        }
                        game::sim::GameSimEvent::HarvestCapacityCheck(check) => {
                            let (loot_preview, requirements) = {
                                let sim = game.sim.read().await;
                                match check.target.clone() {
                                    crate::game::protocol::InteractionTarget::Tile(tile) => {
                                        let loot = sim.world.harvest_loot_preview_at(tile);
                                        let reqs = sim.world.harvest_node_def_at(tile).map(|d| {
                                            (
                                                d.required_level,
                                                d.skill.id(),
                                                d.skill.display_name(),
                                                d.skill.required_tool_kind(),
                                            )
                                        });
                                        (loot, reqs)
                                    }
                                }
                            };

                            let Some(loot) = loot_preview else {
                                let message = "No harvest loot available".to_string();
                                if let Some(msg) = {
                                    let mut sim = game.sim.write().await;
                                    sim.reject_harvest_capacity_check(
                                        check.player_id,
                                        check.target.clone(),
                                        message.clone(),
                                    )
                                } {
                                    game.hub.broadcast(msg);
                                }
                                game.hub.broadcast(crate::game::protocol::ServerMsg::Error { message });
                                continue;
                            };

                            // Server-authoritative harvest lock: require the skill
                            // level AND an equipped tool whose tier covers the node.
                            // On success, capture the character's level and axe tier
                            // to seed the per-swing success scaling.
                            let mut harvest_power: (u32, u32) = (0, 0);
                            if let Some((req_level, skill_id, skill_display, tool_kind)) = requirements {
                                let reject_message = match game::sim::equipment::check_harvest_gate(
                                    &db,
                                    check.character_id,
                                    req_level,
                                    skill_id,
                                    skill_display,
                                    tool_kind,
                                )
                                .await
                                {
                                    Ok(game::sim::equipment::HarvestGate::Ok { skill_level, tool_level }) => {
                                        harvest_power = (skill_level, tool_level);
                                        None
                                    }
                                    Ok(game::sim::equipment::HarvestGate::LevelTooLow {
                                        required,
                                        skill_display,
                                    }) => Some(format!(
                                        "You need level {required} {skill_display} to harvest this."
                                    )),
                                    Ok(game::sim::equipment::HarvestGate::ToolMissing {
                                        required_tool_name,
                                    }) => Some(format!(
                                        "This trunk is too hard for your axe - you need a {required_tool_name} or better."
                                    )),
                                    Err(e) => {
                                        warn!(
                                            "harvest gate check failed character_id={} error={:?}",
                                            check.character_id, e
                                        );
                                        Some("failed to check harvest requirements".to_string())
                                    }
                                };

                                if let Some(message) = reject_message {
                                    // The client mirrors this gate locally and shows the
                                    // reason as a right-side drop, so we only broadcast the
                                    // authoritative action-Rejected state — no extra
                                    // ServerMsg::Error, which would double the message.
                                    if let Some(msg) = {
                                        let mut sim = game.sim.write().await;
                                        sim.reject_harvest_capacity_check(
                                            check.player_id,
                                            check.target.clone(),
                                            message,
                                        )
                                    } {
                                        game.hub.broadcast(msg);
                                    }
                                    continue;
                                }
                            }

                            match game::sim::inventory::can_character_inventory_accept_item(
                                &db,
                                check.character_id,
                                loot.item_id,
                                loot.quantity,
                            )
                            .await
                            {
                                Ok(result) if result.can_accept => {
                                    if let Some(msg) = {
                                        let mut sim = game.sim.write().await;
                                        sim.approve_harvest_capacity_check(
                                            check.player_id,
                                            check.target.clone(),
                                            harvest_power.0,
                                            harvest_power.1,
                                        )
                                    } {
                                        game.hub.broadcast(msg);
                                    }
                                }
                                Ok(result) => {
                                    info!(
                                        "harvest action rejected at node because inventory is full character_id={} item_id={} quantity={} slots_used={} slots_total={} additional_slots_required={}",
                                        check.character_id,
                                        result.item_id,
                                        result.quantity,
                                        result.slots_used,
                                        result.slots_total,
                                        result.additional_slots_required
                                    );

                                    let message = "Inventory full".to_string();
                                    if let Some(msg) = {
                                        let mut sim = game.sim.write().await;
                                        sim.reject_harvest_capacity_check(
                                            check.player_id,
                                            check.target.clone(),
                                            message.clone(),
                                        )
                                    } {
                                        game.hub.broadcast(msg);
                                    }
                                    game.hub.broadcast(crate::game::protocol::ServerMsg::Error { message });
                                }
                                Err(e) => {
                                    warn!(
                                        "harvest inventory capacity check failed character_id={} target={:?} error={:?}",
                                        check.character_id,
                                        check.target,
                                        e
                                    );

                                    let message = "failed to check inventory capacity".to_string();
                                    if let Some(msg) = {
                                        let mut sim = game.sim.write().await;
                                        sim.reject_harvest_capacity_check(
                                            check.player_id,
                                            check.target.clone(),
                                            message.clone(),
                                        )
                                    } {
                                        game.hub.broadcast(msg);
                                    }
                                    game.hub.broadcast(crate::game::protocol::ServerMsg::Error { message });
                                }
                            }
                        }
                        game::sim::GameSimEvent::SkillXpGrant(grant) => {
                            let source = Some(grant.source.clone());

                            match game::sim::skills::grant_character_skill_xp(
                                &db,
                                grant.character_id,
                                &grant.skill_id,
                                &grant.display_name,
                                grant.xp_delta,
                            )
                            .await
                            {
                                Ok(result) => {
                                    game.hub.broadcast(crate::game::protocol::ServerMsg::SkillDelta(
                                        game::sim::skills::skill_delta_from_result(result, source),
                                    ));
                                }
                                Err(e) => {
                                    warn!(
                                        "persistent skill xp grant failed character_id={} skill_id={} xp_delta={} error={:?}",
                                        grant.character_id,
                                        grant.skill_id,
                                        grant.xp_delta,
                                        e
                                    );
                                }
                            }
                        }
                        game::sim::GameSimEvent::InventoryGrant(grant) => {
                            match game::sim::inventory::grant_character_item(
                                &db,
                                grant.character_id,
                                &grant.item_id,
                                grant.quantity,
                            )
                            .await
                            {
                                Ok(result) => {
                                    game.hub.broadcast(crate::game::protocol::ServerMsg::HarvestResult(
                                        crate::game::protocol::HarvestResult {
                                            player_id: grant.player_id,
                                            character_id: grant.character_id,
                                            action: grant.action,
                                            target: grant.target.clone(),
                                            node_id: grant.node_id.clone(),
                                            display_name: grant.display_name.clone(),
                                            success: true,
                                            item_id: Some(result.item_id.clone()),
                                            quantity: result.quantity,
                                            inventory_quantity: Some(result.new_quantity),
                                            charges_remaining: grant.charges_remaining,
                                        },
                                    ));

                                    match game::sim::inventory::load_character_inventory_snapshot(
                                        &db,
                                        result.character_id,
                                    )
                                    .await
                                    {
                                        Ok(snapshot) => {
                                            game.hub.broadcast(crate::game::protocol::ServerMsg::InventorySnapshot(snapshot));
                                        }
                                        Err(e) => {
                                            warn!(
                                                "inventory snapshot refresh failed after grant character_id={} item_id={} error={:?}",
                                                result.character_id,
                                                result.item_id,
                                                e
                                            );
                                        }
                                    }
                                }
                                Err(game::sim::inventory::InventoryGrantError::InventoryFull {
                                    item_id,
                                    quantity,
                                    slots_used,
                                    slots_total,
                                }) => {
                                    info!(
                                        "inventory grant rejected because inventory is full character_id={} item_id={} quantity={} slots_used={} slots_total={}",
                                        grant.character_id,
                                        item_id,
                                        quantity,
                                        slots_used,
                                        slots_total
                                    );

                                    game.hub.broadcast(crate::game::protocol::ServerMsg::HarvestResult(
                                        crate::game::protocol::HarvestResult {
                                            player_id: grant.player_id,
                                            character_id: grant.character_id,
                                            action: grant.action,
                                            target: grant.target.clone(),
                                            node_id: grant.node_id.clone(),
                                            display_name: grant.display_name.clone(),
                                            success: false,
                                            item_id: Some(item_id),
                                            quantity: 0,
                                            inventory_quantity: None,
                                            charges_remaining: grant.charges_remaining,
                                        },
                                    ));

                                    game.hub.broadcast(crate::game::protocol::ServerMsg::Error {
                                        message: "Inventory full".to_string(),
                                    });

                                    // Stop the server-driven harvest loop — otherwise it
                                    // would keep rolling and failing every swing.
                                    if let Some(msg) = {
                                        let mut sim = game.sim.write().await;
                                        sim.cancel_player_action(grant.player_id, "Inventory full")
                                    } {
                                        game.hub.broadcast(msg);
                                    }
                                }
                                Err(game::sim::inventory::InventoryGrantError::Db(e)) => {
                                    warn!(
                                        "persistent inventory grant failed character_id={} item_id={} quantity={} error={:?}",
                                        grant.character_id,
                                        grant.item_id,
                                        grant.quantity,
                                        e
                                    );

                                    game.hub.broadcast(crate::game::protocol::ServerMsg::HarvestResult(
                                        crate::game::protocol::HarvestResult {
                                            player_id: grant.player_id,
                                            character_id: grant.character_id,
                                            action: grant.action,
                                            target: grant.target,
                                            node_id: grant.node_id,
                                            display_name: grant.display_name,
                                            success: true,
                                            item_id: Some(grant.item_id),
                                            quantity: grant.quantity,
                                            inventory_quantity: None,
                                            charges_remaining: grant.charges_remaining,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    tokio::spawn({
        let game = game.clone();
        async move {
            let dt = std::time::Duration::from_millis((1000 / snapshot_hz.max(1)) as u64);
            let mut interval = tokio::time::interval(dt);

            loop {
                interval.tick().await;
                let sim = game.sim.read().await;
                let snap = sim.snapshot();
                game.hub.broadcast(crate::game::protocol::ServerMsg::Snapshot(snap));
            }
        }
    });
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}
