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
    last_follow_target: Option<TilePos>,
}

/// Follow server-authoritative movement without visually backtracking.
///
/// The server snapshot contains the last reached tile, next server step, and
/// current server-approved goal. The local client uses those snapshots for
/// presentation, but it should not chase a next step that is already behind the
/// local visual position. That small "chase the previous server step" case is
/// what caused the one-tile bounce when spam-clicking.
///
/// If the local visual has already moved past the server's next step relative to
/// the server-approved goal, we continue toward the server goal instead of
/// moving backward to the older next step. Server authority is still preserved by
/// the hard correction threshold for real desyncs.
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
        local_state.last_follow_target = None;
        return;
    };

    let local_tile = world_to_tile(player_feet_world(&xform));
    status.local_tile = Some(local_tile);

    let Some(server_tile) = status.server_tile else {
        status.drift_tiles = None;
        local_state.last_follow_target = None;
        return;
    };

    let server_drift = manhattan_distance(local_tile, server_tile);
    status.drift_tiles = Some(server_drift);

    let raw_follow_target = if status.server_moving {
        status.server_next_tile.unwrap_or(server_tile)
    } else {
        server_tile
    };

    if server_drift >= HARD_CORRECT_THRESHOLD {
        let center = tile_to_world_center(server_tile);
        xform.translation.x = center.x;
        xform.translation.y = center.y + FOOT_OFFSET_Y;
        path.tiles.clear();
        commands.entity(entity).remove::<StepTo>();
        status.correction_count += 1;
        local_state.last_follow_target = Some(server_tile);
        warn!(
            "game net hard-corrected local player to server tile {},{} server_drift={}",
            server_tile.x, server_tile.y, server_drift
        );
        return;
    }

    let follow_target = choose_visual_follow_target(
        local_tile,
        raw_follow_target,
        status.server_goal,
        status.server_moving,
        world.as_deref(),
    );

    let follow_drift = manhattan_distance(local_tile, follow_target);

    if follow_drift == 0 {
        local_state.last_follow_target = Some(follow_target);
        return;
    }

    let current_step = step_opt.map(|s| s.0);
    let path_goal = path.tiles.back().copied().or(current_step);
    let already_following_target = path_goal == Some(follow_target);
    let follow_target_changed = local_state.last_follow_target != Some(follow_target);

    if follow_target_changed || !already_following_target {
        let next_path = find_visual_path(local_tile, follow_target, world.as_deref());

        path.tiles = next_path;
        commands.entity(entity).remove::<StepTo>();
        local_state.last_follow_target = Some(follow_target);
        debug!(
            "game net visual follow target {},{} from {},{} server_tile={},{} raw_next={:?} goal={:?} moving={} server_drift={} follow_drift={}",
            follow_target.x,
            follow_target.y,
            local_tile.x,
            local_tile.y,
            server_tile.x,
            server_tile.y,
            status.server_next_tile,
            status.server_goal,
            status.server_moving,
            server_drift,
            follow_drift
        );
    }
}

fn choose_visual_follow_target(
    local_tile: TilePos,
    raw_follow_target: TilePos,
    server_goal: Option<TilePos>,
    server_moving: bool,
    world: Option<&WorldGrid>,
) -> TilePos {
    let Some(goal) = server_goal else {
        return raw_follow_target;
    };

    if !server_moving {
        return raw_follow_target;
    }

    // If the raw server next step is closer to the goal than the local visual,
    // following it is forward progress. Use it.
    let local_goal_dist = manhattan_distance(local_tile, goal);
    let target_goal_dist = manhattan_distance(raw_follow_target, goal);
    if target_goal_dist < local_goal_dist {
        return raw_follow_target;
    }

    // If local visual is already at/near/past that step relative to the current
    // goal, do not visually walk backward. Continue one local step toward the
    // server-approved goal instead. This is presentation-only; the server still
    // owns tile state and can hard-correct large desyncs.
    next_step_toward(local_tile, goal, world).unwrap_or(raw_follow_target)
}

fn find_visual_path(from: TilePos, to: TilePos, world: Option<&WorldGrid>) -> VecDeque<TilePos> {
    if let Some(world) = world {
        let found = world.find_path_bfs(from, to);
        if !found.is_empty() || from == to {
            return found;
        }
    }

    manhattan_path(from, to)
}

fn next_step_toward(from: TilePos, goal: TilePos, world: Option<&WorldGrid>) -> Option<TilePos> {
    if from == goal {
        return Some(goal);
    }

    if let Some(world) = world {
        let found = world.find_path_bfs(from, goal);
        if let Some(first) = found.front().copied() {
            return Some(first);
        }
    }

    manhattan_path(from, goal).front().copied()
}

fn manhattan_distance(a: TilePos, b: TilePos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
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
