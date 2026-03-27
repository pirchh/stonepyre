use sqlx::PgPool;

use crate::config::Config;
use crate::game::GameRuntime;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Config,
    pub pool: PgPool,
    // Back-compat: a lot of your code expects `state.db`
    pub db: PgPool,

    // in-memory game runtime (hub + sim)
    pub game: GameRuntime,
}

impl AppState {
    pub fn new(cfg: Config, pool: PgPool, game: GameRuntime) -> Self {
        Self {
            cfg,
            db: pool.clone(),
            pool,
            game,
        }
    }
}