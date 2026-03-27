use std::sync::Arc;

use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::config::MarketConfig;
use crate::sim::company_factory::CompanyFactory;
use crate::sim::engine::SimCache;

#[derive(Clone)]
pub struct AppState {
    pub cfg: MarketConfig,
    pub pool: PgPool,

    // In-memory cache to avoid hammering DB every tick
    pub sim_cache: Arc<RwLock<SimCache>>,

    // Runtime IPO factory (loaded once at startup if assets exist).
    // None => IPO spawning disabled.
    pub company_factory: Arc<RwLock<Option<CompanyFactory>>>,
}

impl AppState {
    pub fn new(cfg: MarketConfig, pool: PgPool) -> Self {
        Self {
            cfg,
            pool,
            sim_cache: Arc::new(RwLock::new(SimCache::default())),
            company_factory: Arc::new(RwLock::new(None)),
        }
    }
}