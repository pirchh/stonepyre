use bevy::prelude::*;

use stonepyre_engine::plugins::world::{LogicalPos2d, Player};
use stonepyre_world::{world3d_to_tile, TILE_SIZE};

use super::status::GameNetStatus;

/// Beyond this the local view snaps instantly (genuine desync / teleport).
const HARD_SNAP_THRESHOLD: f32 = TILE_SIZE * 3.0;

/// Below this gap we fully **trust the local prediction** and apply no correction
/// at all — that's what keeps movement smooth.
///
/// With the deterministic shared integrator (A.1), server-authoritative collision
/// (A.2), and matched speed, the client's prediction matches the server; the only
/// residual is latency — the server is ~one tick behind us along the *same* path,
/// so the gap is real-but-correct and the server converges back to us (no drift
/// accumulation, no stop-teleport, walls already clamped identically). Pulling the
/// 60fps render toward the 10Hz-stale snapshot every frame just causes a sawtooth
/// tug (worst right after a turn, when the snapshot still reflects the old
/// direction). So we don't. Only a *sustained* gap past this threshold is a
/// genuine mispredict (e.g. recovering from dropped packets) worth converging.
const SOFT_CORRECT_THRESHOLD: f32 = TILE_SIZE * 1.0;

/// Gentle per-frame blend toward authority while a genuine (1–3 tile) gap exists.
/// Only runs in that rare band, so it never affects normal smooth movement.
const SOFT_CORRECT_RATE: f32 = 0.20;

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

    // Below the soft threshold: trust the prediction, do nothing (smooth).
    // Genuine 1–3 tile gap: blend gently toward authority.
    if error > SOFT_CORRECT_THRESHOLD {
        let new_pos = local.lerp(auth, SOFT_CORRECT_RATE);
        logical.0 = new_pos;
        xform.translation.x = new_pos.x;
        xform.translation.z = new_pos.y;
    }
}
