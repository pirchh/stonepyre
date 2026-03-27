mod config;
mod error;
mod http;
mod state;
mod game;

use axum::Router;
use sqlx::{Connection, Row};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};

use crate::{config::Config, state::AppState};

const MARKET_SIM_LOCK_KEY: i64 = 9_007_199_254_740_993;
const GAME_SIM_LOCK_KEY: i64 = 9_007_199_254_740_994; // different key

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let log_level = std::env::var("SERVER_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt().with_env_filter(log_level).init();

    let cfg = Config::from_env()?;
    info!("stonepyre_server starting...");
    info!("bind={}", cfg.bind_addr);
    info!("db={}", cfg.database_url);

    // Server-owned pool
    let pool = sqlx::PgPool::connect(&cfg.database_url).await?;

    // In-memory game runtime
    let game = game::GameRuntime::new();

    let state = AppState::new(cfg.clone(), pool.clone(), game.clone());

    // ---------------------------------------------------------
    // Prevent double ticks: market sim lock held on a dedicated connection
    // ---------------------------------------------------------
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

    // Keep lock conn alive
    let _keep_market_lock_conn_alive = market_lock_conn;
    // ---------------------------------------------------------

    // ---------------------------------------------------------
    // Prevent double ticks: game sim lock held on a dedicated connection
    // ---------------------------------------------------------
    let mut game_lock_conn = sqlx::PgConnection::connect(&cfg.database_url).await?;
    let game_got_lock: bool = sqlx::query("SELECT pg_try_advisory_lock($1) AS ok")
        .bind(GAME_SIM_LOCK_KEY)
        .fetch_one(&mut game_lock_conn)
        .await?
        .try_get("ok")?;

    if game_got_lock {
        let tick_hz: u32 = std::env::var("GAME_TICK_HZ")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let snapshot_hz: u32 = std::env::var("GAME_SNAPSHOT_HZ")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);

        info!(
            "game sim lock acquired; starting game loops tick_hz={} snapshot_hz={}",
            tick_hz, snapshot_hz
        );

        start_game_loops(game.clone(), tick_hz, snapshot_hz);
    } else {
        warn!("game sim lock NOT acquired; another process is running the game loop. server will NOT tick the game.");
    }

    let _keep_game_lock_conn_alive = game_lock_conn;
    // ---------------------------------------------------------

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

fn start_game_loops(game: game::GameRuntime, tick_hz: u32, snapshot_hz: u32) {
    // 1) Sim tick loop
    tokio::spawn({
        let game = game.clone();
        async move {
            let dt = std::time::Duration::from_millis((1000 / tick_hz.max(1)) as u64);
            let mut interval = tokio::time::interval(dt);

            loop {
                interval.tick().await;
                let mut sim = game.sim.write().await;
                sim.step();
            }
        }
    });

    // 2) Snapshot broadcast loop (slower)
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