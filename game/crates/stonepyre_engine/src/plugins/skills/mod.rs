use bevy::prelude::*;
use std::time::Duration;

// Keep your clip/request API here so animation + interaction can depend on skills
// without importing individual skill files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnimClip {
    Woodcutting,
    // Future: Mining, Fishing, etc.
}

#[derive(Clone, Debug)]
pub enum RequestedAnimMode {
    /// Plays once then auto-removes RequestedAnim.
    OneShot { timer: Timer },

    /// Keeps playing until something else removes RequestedAnim.
    Loop { timer: Timer },
}

impl RequestedAnimMode {
    pub fn tick(&mut self, dt: Duration) {
        match self {
            RequestedAnimMode::OneShot { timer } => {
                timer.tick(dt);
            }
            RequestedAnimMode::Loop { timer } => {
                timer.tick(dt);
            }
        }
    }

    pub fn just_finished(&self) -> bool {
        match self {
            RequestedAnimMode::OneShot { timer } => timer.just_finished(),
            RequestedAnimMode::Loop { timer } => timer.just_finished(),
        }
    }

    pub fn is_one_shot(&self) -> bool {
        matches!(self, RequestedAnimMode::OneShot { .. })
    }
}

#[derive(Component, Debug)]
pub struct RequestedAnim {
    pub clip: AnimClip,
    pub mode: RequestedAnimMode,
}

// --- Modules ---
pub mod harvest;
pub mod levels;
pub mod woodcutting;

// --- Re-exports expected by other modules ---
//
// HarvestNodeDef is content-owned now; re-export it for convenience.
pub use stonepyre_content::objects::HarvestNodeDef;

pub use harvest::{HarvestDb, HarvestNode, sync_harvest_node_visibility, tick_harvest_regen};
pub use levels::SkillLevels;