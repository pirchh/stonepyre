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
    pub fn new() -> Self {
        let hub = GameHub::new();
        let sim = Arc::new(RwLock::new(GameSim::new()));
        Self { hub, sim }
    }
}