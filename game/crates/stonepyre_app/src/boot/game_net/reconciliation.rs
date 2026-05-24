use bevy::prelude::*;
use std::collections::VecDeque;

use stonepyre_engine::plugins::{
    movement::StepTo,
    world::{Player, TilePath},
};
use stonepyre_world::{tile_to_world3d, world3d_to_tile, TilePos, WorldGrid};

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

    let local_tile = world3d_to_tile(xform.translation);
    status.local_tile = Some(local_tile);

    // If the server just sent us an authoritative path, apply it — but trim any
    // tiles the client has already visually passed. The server path starts from
    // the server's last discrete tile, which may be several tiles behind the
    // client's current smooth position. Applying it without trimming would cause
    // the client to walk backward to the server's position before continuing forward.
    //
    // Strategy: find the first tile in the server path that is no farther from
    // the goal than the client currently is. Everything before that is "already
    // traversed" from the client's perspective.
    if let Some((goal, server_tiles)) = status.pending_server_path.take() {
        status.pending_path_confirmations = status.pending_path_confirmations.saturating_sub(1);
        // Find the tile in the server path closest to the client's current position
        // and start there. This handles two cases cleanly:
        //   - Client is at/near the path start (no prediction): start_idx ≈ 0
        //   - Client somehow ended up mid-path: skip tiles already behind it
        //
        // We use "closest spatial tile" rather than "goal-distance comparison"
        // because goal-distance breaks on detour paths (BFS around an obstacle
        // may pass through tiles with *larger* Manhattan distance to goal than
        // the client has, causing the comparison to incorrectly skip them).
        let start_idx = if server_tiles.is_empty() {
            0
        } else {
            server_tiles
                .iter()
                .enumerate()
                .min_by_key(|(_, t)| manhattan_distance(local_tile, **t))
                .map(|(i, _)| i)
                .unwrap_or(0)
        };

        let new_path: std::collections::VecDeque<TilePos> =
            server_tiles[start_idx..].iter().copied().collect();

        path.tiles = new_path;
        commands.entity(entity).remove::<StepTo>();
        // Record the confirmed goal so the follow-target logic below won't
        // overwrite this path on the next snapshot tick.
        local_state.last_follow_target = Some(goal);
        return;
    }

    let Some(server_tile) = status.server_tile else {
        status.drift_tiles = None;
        local_state.last_follow_target = None;
        return;
    };

    // Use the server's sub-tile progress to pick a more accurate reference
    // position for drift measurement. If the server is more than halfway through
    // stepping to next_tile, treat next_tile as the effective server position so
    // we don't over-count drift against a tile the server has almost left.
    let effective_server_tile = if status.server_moving
        && status.server_move_progress >= 0.5
    {
        status.server_next_tile.unwrap_or(server_tile)
    } else {
        server_tile
    };

    let server_drift = manhattan_distance(local_tile, effective_server_tile);
    status.drift_tiles = Some(server_drift);

    let raw_follow_target = if status.server_moving {
        status.server_next_tile.unwrap_or(server_tile)
    } else {
        server_tile
    };

    if server_drift >= HARD_CORRECT_THRESHOLD {
        let center = tile_to_world3d(effective_server_tile);
        xform.translation.x = center.x;
        xform.translation.y = 0.0;
        xform.translation.z = center.z;
        path.tiles.clear();
        commands.entity(entity).remove::<StepTo>();
        status.correction_count += 1;
        local_state.last_follow_target = Some(effective_server_tile);
        warn!(
            "game net hard-corrected local player to server tile {},{} (effective) server_drift={}",
            effective_server_tile.x, effective_server_tile.y, server_drift
        );
        return;
    }

    // MoveTo was sent but PathConfirmed hasn't arrived yet (~100ms RTT window).
    // Don't run heuristics: near obstacles, BFS toward server_next_tile picks the
    // wrong side of the tree. The client finishes its current step and waits.
    // Counter tracks N in-flight responses for rapid clicks so heuristics stay
    // suppressed until the very last PathConfirmed has been applied.
    if status.pending_path_confirmations > 0 {
        return;
    }

    // If the client is already walking a confirmed path toward the server's current
    // goal, trust it — do not overwrite it with the follow-target heuristic.
    // Without this guard, every snapshot tick (~100ms) the reconciler would replace
    // the multi-tile confirmed path with a 1-tile hop to server_next_tile because
    // path_goal (= server goal) ≠ follow_target (= server_next_tile).
    let current_step = step_opt.map(|s| s.0);
    let path_goal = path.tiles.back().copied().or(current_step);
    if let (Some(pg), Some(sg)) = (path_goal, status.server_goal) {
        if pg == sg && !path.tiles.is_empty() {
            // Active confirmed path still in progress — let follow_path_to_next_tile
            // drive it. Drift is still measured above for display purposes.
            return;
        }
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
