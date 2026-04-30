use bevy::prelude::*;

use stonepyre_engine::plugins::world::{GridPos, InteractableKind};

use super::status::GameNetStatus;

const TREE_AVAILABLE_COLOR: Color = Color::srgb(0.2, 0.8, 0.2);
const TREE_DEPLETED_COLOR: Color = Color::srgb(0.34, 0.31, 0.24);

/// Presentation-only bridge for server-owned harvest node state.
///
/// The server remains authoritative for whether a node can be harvested. This
/// system only mirrors the latest `WorldSnapshot.harvest_nodes` state onto the
/// local demo-world entities. The depleted tint is a temporary stand-in for the
/// future stump sprite swap.
pub fn sync_harvest_node_visuals_from_server(
    status: Res<GameNetStatus>,
    mut tree_q: Query<(&GridPos, &mut Sprite), With<InteractableKind>>,
) {
    if status.harvest_nodes.is_empty() {
        return;
    }

    for (grid_pos, mut sprite) in &mut tree_q {
        let Some(node) = status.harvest_nodes.iter().find(|node| node.tile == grid_pos.0) else {
            continue;
        };

        sprite.color = if node.depleted {
            TREE_DEPLETED_COLOR
        } else {
            TREE_AVAILABLE_COLOR
        };
    }
}
