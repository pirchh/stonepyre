pub mod protocol;
pub mod sim;
pub mod hub;

use std::sync::Arc;
use tokio::sync::RwLock;

use self::{hub::GameHub, sim::GameSim};

#[derive(Clone)]
pub struct GameRuntime {
    pub hub: GameHub,
    pub sim: Arc<RwLock<GameSim>>,
}

impl GameRuntime {
    pub fn new(tick_hz: u32) -> Self {
        let hub = GameHub::new();
        let sim = Arc::new(RwLock::new(GameSim::new(tick_hz)));
        Self { hub, sim }
    }
}
