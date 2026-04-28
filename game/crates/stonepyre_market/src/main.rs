mod config;
mod state;
mod types;

mod api;
mod db;
mod sim;

use axum::Router;
use sqlx::{Connection, Row};
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};
use rand::SeedableRng;

use crate::config::MarketConfig;
use crate::state::AppState;

// ✅ One global lock key shared with stonepyre_server.
// Any stable i64 works; keep it identical in both crates.
const MARKET_SIM_LOCK_KEY: i64 = 9_007_199_254_740_993;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let log_level = std::env::var("MARKET_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    let cfg = MarketConfig::from_env()?;
    info!("stonepyre_market starting...");
    info!("bind={}", cfg.bind_addr);
    info!("tick_ms={}", cfg.tick_ms);

    // DB pool
    let pool = db::connect(&cfg.database_url).await?;

    // ---------------------------------------------------------
    // 🧪 SEED MODE (no sim loop; does not need lock)
    // ---------------------------------------------------------
    let args: Vec<String> = std::env::args().collect();
    let do_seed = args.iter().any(|a| a == "--seed");

    if do_seed {
        info!("running in --seed mode");

        let assets_dir = std::path::PathBuf::from("assets/market");

        let fill_ratio: f64 = std::env::var("MARKET_INIT_FILL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.60);

        let mut rng = rand::rngs::StdRng::seed_from_u64(cfg.sim_seed);

        sim::seed::seed_all(&pool, &mut rng, &assets_dir, fill_ratio).await?;

        info!("seed complete; exiting.");
        return Ok(());
    }
    // ---------------------------------------------------------

    // ---------------------------------------------------------
    // ✅ Prevent double ticks (shared lock with server)
    // Hold advisory lock on a dedicated connection for lifetime.
    // ---------------------------------------------------------
    let mut lock_conn = sqlx::PgConnection::connect(&cfg.database_url).await?;
    let got_lock: bool = sqlx::query("SELECT pg_try_advisory_lock($1) AS ok")
        .bind(MARKET_SIM_LOCK_KEY)
        .fetch_one(&mut lock_conn)
        .await?
        .try_get("ok")?;

    if !got_lock {
        warn!(
            "market sim lock NOT acquired; another process is running the sim loop. exiting."
        );
        return Ok(());
    }

    info!("market sim lock acquired; sim loop enabled");
    let _keep_lock_conn_alive = lock_conn;
    // ---------------------------------------------------------

    // Shared app state
    let state = AppState::new(cfg.clone(), pool);

    // Start simulator loop
    sim::start_sim_loop(state.clone());

    // Build API
    let app: Router = api::router(state);

    // Serve
    let addr: SocketAddr = cfg.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    warn!("stonepyre_market stopped");
    Ok(())
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}