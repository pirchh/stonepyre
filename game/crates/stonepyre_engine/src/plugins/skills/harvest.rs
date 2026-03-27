use bevy::prelude::*;

use stonepyre_content::objects::{HarvestDefs, HarvestNodeDef};

/// Engine wrapper around content harvest defs.
/// Content is authoritative; engine reads it.
#[derive(Resource, Clone)]
pub struct HarvestDb(pub HarvestDefs);

impl HarvestDb {
    pub fn from_defs(defs: HarvestDefs) -> Self {
        Self(defs)
    }

    pub fn get(&self, id: &str) -> Option<&HarvestNodeDef> {
        self.0.nodes.get(id)
    }
}

/// Runtime state component on an entity in the world (a specific tree/rock/etc).
#[derive(Component, Debug)]
pub struct HarvestNode {
    pub def_id: &'static str,

    pub charges_left: i32,
    pub charges_max: i32,

    /// +1 charge every respawn_seconds.
    /// IMPORTANT: we reset this when consuming so regen doesn't happen instantly.
    pub regen_timer: Timer,

    pub depleted: bool,
}

impl HarvestNode {
    pub fn from_def_id(def_id: &'static str, defs: &HarvestDefs) -> Self {
        let (charges, secs) = if let Some(def) = defs.nodes.get(def_id) {
            (def.charges, def.respawn_seconds)
        } else {
            (1, 10.0)
        };

        Self {
            def_id,
            charges_left: charges,
            charges_max: charges,
            regen_timer: Timer::from_seconds(secs, TimerMode::Repeating),
            depleted: false,
        }
    }

    pub fn is_depleted(&self) -> bool {
        self.depleted || self.charges_left <= 0
    }

    /// Consume one charge and reset regen timer so regen waits a full interval.
    pub fn consume_one(&mut self) {
        if self.charges_left > 0 {
            self.charges_left -= 1;
        }

        if self.charges_left <= 0 {
            self.charges_left = 0;
            self.depleted = true;
        }

        // Start waiting from 0 now.
        self.regen_timer.reset();
    }
}

pub fn tick_harvest_regen(time: Res<Time>, mut q: Query<&mut HarvestNode>) {
    for mut node in q.iter_mut() {
        // If full, keep parked/reset so we never have an "instant just_finished" after a consume.
        if node.charges_left >= node.charges_max {
            node.charges_left = node.charges_max;
            node.depleted = false;
            node.regen_timer.reset();
            continue;
        }

        // Needs regen: tick
        node.regen_timer.tick(time.delta());

        if node.regen_timer.just_finished() {
            let before = node.charges_left;
            node.charges_left += 1;

            if node.charges_left >= node.charges_max {
                node.charges_left = node.charges_max;
                node.depleted = false;
                node.regen_timer.reset(); // park again
            } else {
                node.depleted = node.charges_left <= 0;
                // repeating timer continues toward next +1
            }

            info!(
                "[harvest] regen {}: {}/{} (was {})",
                node.def_id, node.charges_left, node.charges_max, before
            );
        }
    }
}

pub fn sync_harvest_node_visibility(mut q: Query<(&HarvestNode, &mut Visibility)>) {
    for (node, mut vis) in q.iter_mut() {
        *vis = if node.is_depleted() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}