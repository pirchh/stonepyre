use bevy::prelude::*;
use std::collections::VecDeque;

use stonepyre_engine::plugins::{
    movement::StepTo,
    world::{player_feet_world, Player, TilePath, FOOT_OFFSET_Y},
};
use stonepyre_world::{tile_to_world_center, world_to_tile, TilePos, WorldGrid};

use super::status::GameNetStatus;

const HARD_CORRECT_THRESHOLD: i32 = 10;

#[derive(Default)]
pub(crate) struct ReconcileLocalState {
    last_server_tile: Option<TilePos>,
}

/// Follow the authoritative server tile smoothly instead of running an independent
/// local movement simulation.
///
/// In networked play, WalkHere clicks are sent to the server and the normal local
/// planner is cleared. This system then converts server tile snapshots into a short
/// local TilePath so the local player animates toward the authoritative position.
/// Hard snapping is reserved for truly large desyncs.
pub fn reconcile_local_player_to_server(
    mut commands: Commands,
    mut status: ResMut<GameNetStatus>,
    mut local_state: Local<ReconcileLocalState>,
    world: Option<Res<WorldGrid>>,
    mut player_q: Query<(Entity, &mut Transform, &mut TilePath, Option<&StepTo>), With<Player>>,
) {
    let Ok((entity, mut xform, mut path, step_opt)) = player_q.single_mut() else {
        status.local_tile = None;
        status.drift_tiles = None;
        local_state.last_server_tile = None;
        return;
    };

    let local_tile = world_to_tile(player_feet_world(&xform));
    status.local_tile = Some(local_tile);

    let Some(server_tile) = status.server_tile else {
        status.drift_tiles = None;
        local_state.last_server_tile = None;
        return;
    };

    let drift = (local_tile.x - server_tile.x).abs() + (local_tile.y - server_tile.y).abs();
    status.drift_tiles = Some(drift);

    if drift == 0 {
        local_state.last_server_tile = Some(server_tile);
        return;
    }

    if drift >= HARD_CORRECT_THRESHOLD {
        let center = tile_to_world_center(server_tile);
        xform.translation.x = center.x;
        xform.translation.y = center.y + FOOT_OFFSET_Y;
        path.tiles.clear();
        commands.entity(entity).remove::<StepTo>();
        status.correction_count += 1;
        local_state.last_server_tile = Some(server_tile);
        warn!(
            "game net hard-corrected local player to server tile {},{} drift={}",
            server_tile.x, server_tile.y, drift
        );
        return;
    }

    let server_tile_changed = local_state.last_server_tile != Some(server_tile);
    let moving = !path.tiles.is_empty() || step_opt.is_some();
    let current_step = step_opt.map(|s| s.0);
    let path_goal = path.tiles.back().copied().or(current_step);
    let already_following_server = path_goal == Some(server_tile);

    // If the server has advanced to a new authoritative tile, or if we are idle but
    // still not on the server tile, create a short local path toward the server.
    // This is not prediction; it is presentation of the authoritative snapshot stream.
    if server_tile_changed || (!moving && drift > 0) || (!already_following_server && drift > 1) {
        let next_path = if let Some(world) = world.as_ref() {
            let found = world.find_path_bfs(local_tile, server_tile);
            if found.is_empty() && local_tile != server_tile {
                manhattan_path(local_tile, server_tile)
            } else {
                found
            }
        } else {
            manhattan_path(local_tile, server_tile)
        };

        path.tiles = next_path;
        commands.entity(entity).remove::<StepTo>();
        local_state.last_server_tile = Some(server_tile);
        debug!(
            "game net following server tile {},{} from {},{} drift={}",
            server_tile.x, server_tile.y, local_tile.x, local_tile.y, drift
        );
    }
}

fn manhattan_path(from: TilePos, to: TilePos) -> VecDeque<TilePos> {
    let mut out = VecDeque::new();
    let mut cur = from;

    while cur.x != to.x {
        cur.x += if to.x > cur.x { 1 } else { -1 };
        out.push_back(cur);
    }

    while cur.y != to.y {
        cur.y += if to.y > cur.y { 1 } else { -1 };
        out.push_back(cur);
    }

    out
}
