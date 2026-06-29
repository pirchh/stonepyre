use bevy::prelude::*;

use stonepyre_engine::plugins::world::{LogicalPos2d, Player};
use stonepyre_world::{slide_move, world3d_to_tile, WorldGrid, TILE_SIZE};

use super::status::GameNetStatus;

/// Beyond this the local view snaps instantly (genuine desync / teleport).
const HARD_SNAP_THRESHOLD: f32 = TILE_SIZE * 3.0;

/// Inside this gap we trust the prediction and apply NO correction — it's pure
/// latency (we render ~one tick ahead of the authoritative position, which is
/// correct and keeps input responsive). The deadzone is what keeps the 10Hz
/// snapshot from sawtoothing the 60fps render in normal play.
const CORRECT_DEADZONE: f32 = TILE_SIZE * 0.2;

/// Past the deadzone we converge toward authority at a rate that EASES IN with the
/// size of the gap (0 at the deadzone, ramping to `MAX_CORRECT_RATE` once this much
/// past it). The smooth ramp is the key: a genuine offset — e.g. after a hard
/// wiggle near a tree — heals gently instead of snapping at a hard threshold or
/// lingering just under one.
const CORRECT_RAMP: f32 = TILE_SIZE * 1.0;
const MAX_CORRECT_RATE: f32 = 0.25;

/// Reconcile the locally-predicted player against the server's authoritative
/// position.
///
/// The client predicts movement locally every frame (engine `wasd_movement`) and
/// we **render that prediction directly**. Because the prediction is deterministic
/// and collision-identical to the server, it's almost always right, so this system
/// usually does nothing but update the drift HUD. It steps in only as a safety net:
///
/// - **Genuine 1–3 tile gap** (a real mispredict, e.g. after packet loss): gently
///   blend the local view back toward authority.
/// - **> 3 tiles**: hard-snap (real desync / teleport).
///
/// Deliberately *not* a per-frame pull toward the server — that re-introduces the
/// 10Hz sawtooth jank. The deterministic foundation (A.1/A.2/B) is what lets the
/// reconciler stay this light.
pub fn reconcile_local_player_to_server(
    world: Option<Res<WorldGrid>>,
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<(&mut Transform, &mut LogicalPos2d), With<Player>>,
) {
    let Some(server_pos) = status.server_pos else {
        return;
    };

    let Ok((mut xform, mut logical)) = player_q.single_mut() else {
        status.local_tile = None;
        status.drift_tiles = None;
        return;
    };

    let auth = Vec2::new(server_pos[0], server_pos[1]);

    // Wait for a real authoritative position before committing — the server sends
    // (0, 0) for the first few snapshots while it loads the character from the DB.
    if !status.initial_sync_done {
        if auth.length_squared() < 1.0 {
            return;
        }
        logical.0 = auth;
        xform.translation.x = auth.x;
        xform.translation.z = auth.y;
        status.initial_sync_done = true;
        info!("reconcile initial-sync: placed at ({:.1}, {:.1})", auth.x, auth.y);
        return;
    }

    let local = logical.0;
    let error = (auth - local).length();
    status.drift_tiles = Some((error / TILE_SIZE).round() as i32);
    status.local_tile = Some(world3d_to_tile(xform.translation));

    // Hard snap for genuine desyncs / teleports.
    if error > HARD_SNAP_THRESHOLD {
        logical.0 = auth;
        xform.translation.x = auth.x;
        xform.translation.z = auth.y;
        status.correction_count += 1;
        warn!(
            "reconcile hard-snap: error={:.1} wu ({:.1} tiles)",
            error,
            error / TILE_SIZE,
        );
        return;
    }

    // Inside the deadzone: trust the prediction, do nothing (pure latency, smooth).
    // Past it: converge toward authority at a rate that eases in with the gap, so a
    // genuine offset (e.g. after a hard wiggle near a tree) heals gently rather than
    // snapping at a hard edge or lingering just under it. COLLISION-AWARE — it
    // slides *around* solids along a legal path rather than lerping through them.
    if error > CORRECT_DEADZONE {
        let ramp = ((error - CORRECT_DEADZONE) / CORRECT_RAMP).clamp(0.0, 1.0);
        let rate = ramp * MAX_CORRECT_RATE;
        let step = (auth - local) * rate;
        let out = match world.as_ref() {
            Some(w) => slide_move([local.x, local.y], [step.x, step.y], |t| w.is_blocked(t)),
            None => [local.x + step.x, local.y + step.y],
        };
        let new_pos = Vec2::new(out[0], out[1]);
        logical.0 = new_pos;
        xform.translation.x = new_pos.x;
        xform.translation.z = new_pos.y;
    }
}
