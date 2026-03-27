use bevy::prelude::*;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[derive(Resource)]
pub struct GameRng(pub StdRng);

impl GameRng {
    pub fn from_seed(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }

    pub fn roll_f32(&mut self) -> f32 {
        self.0.r#gen::<f32>()
    }
}

impl Default for GameRng {
    fn default() -> Self {
        // Deterministic seed for now (you can swap to time-based later)
        Self::from_seed(1337)
    }
}