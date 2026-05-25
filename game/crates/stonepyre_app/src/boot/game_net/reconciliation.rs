use bevy::prelude::*;

use stonepyre_engine::plugins::world::{LogicalPos2d, Player};
use stonepyre_world::TILE_SIZE;

use super::status::GameNetStatus;

/// 3 tiles — any error beyond this snaps immediately (desync / server correction).
const HARD_SNAP_THRESHOLD: f32 = TILE_SIZE * 3.0;

/// Lerp factor applied each frame while the player is standing still.
/// At 60fps, 0.15 closes a 1-tile gap in ~10 frames (~167ms) — fast enough
/// to feel instant, slow enough to be invisible.
const IDLE_LERP_FACTOR: f32 = 0.15;

/// Reconcile the local player position against the server's authoritative position.
///
/// Two-mode strategy:
///
/// **While moving** — client is ground truth. No correction at all. The server
/// is always 100ms+ behind due to tick rate, so any lerp toward its position
/// would fight the player's own input and create visible jitter.
///
/// **While idle** — gently lerp toward the server position at `IDLE_LERP_FACTOR`
/// per frame. This converges the tick-rate drift that accumulates during movement
/// (different step sizes: client 60fps vs server 10hz) without any visual
/// artifact, because the player isn't going anywhere anyway.
///
/// Hard snap fires in both modes when error exceeds 3 tiles — genuine desync.
pub fn reconcile_local_player_to_server(
    keyboard: Res<ButtonInput<KeyCode>>,
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

    let local = logical.0;
    let target = Vec2::new(server_pos[0], server_pos[1]);
    let error = (target - local).length();

    status.drift_tiles = Some((error / TILE_SIZE).round() as i32);
    status.local_tile = Some(stonepyre_world::world3d_to_tile(xform.translation));

    if error > HARD_SNAP_THRESHOLD {
        logical.0 = target;
        xform.translation.x = target.x;
        xform.translation.z = target.y;
        status.correction_count += 1;
        warn!(
            "reconcile hard-snap: error={:.1} wu ({:.1} tiles)",
            error,
            error / TILE_SIZE,
        );
        return;
    }

    // Check whether any movement key is currently held.
    let is_moving =
        keyboard.pressed(KeyCode::KeyW)    || keyboard.pressed(KeyCode::ArrowUp)   ||
        keyboard.pressed(KeyCode::KeyS)    || keyboard.pressed(KeyCode::ArrowDown) ||
        keyboard.pressed(KeyCode::KeyA)    || keyboard.pressed(KeyCode::ArrowLeft) ||
        keyboard.pressed(KeyCode::KeyD)    || keyboard.pressed(KeyCode::ArrowRight);

    if !is_moving && error > 0.5 {
        // Idle — nudge toward server so tick-rate drift converges to zero.
        let new_pos = local.lerp(target, IDLE_LERP_FACTOR);
        logical.0 = new_pos;
        xform.translation.x = new_pos.x;
        xform.translation.z = new_pos.y;
    }
    // Moving — trust the client completely. No correction, no jitter.
}
