use bevy::prelude::*;

use stonepyre_engine::plugins::input::InputBindings;
use stonepyre_engine::plugins::world::{LogicalPos2d, Player};
use stonepyre_world::TILE_SIZE;

use super::status::GameNetStatus;

/// 3 tiles — any error beyond this snaps immediately (genuine desync / wall-phasing).
const HARD_SNAP_THRESHOLD: f32 = TILE_SIZE * 3.0;

/// Lerp factor per frame while idle correction is active.
/// 0.05 at 60fps closes a 1-tile gap in ~0.3s — slow enough to be invisible.
const IDLE_LERP_FACTOR: f32 = 0.05;

/// Seconds the player must be fully idle before correction kicks in.
/// Gives the server time to process the stop command and stabilise its position
/// before we try to reconcile against it.
/// 10hz tick (100ms) + ~50ms RTT + margin = ~0.35s.
const IDLE_SETTLE_SECS: f32 = 0.35;

/// Dead zone: don't apply any idle correction for errors smaller than this.
/// Tick-rate lag alone causes ~10wu of drift at 1.6 tiles/sec — well under this
/// threshold. This means small sub-tile positional differences are simply
/// accepted, so the player can stand on tile edges/corners without being
/// nudged toward the server's resting position.
const IDLE_CORRECT_MIN_ERROR: f32 = TILE_SIZE * 0.5; // 32wu — half a tile

/// Reconcile the local player position against the server's authoritative position.
///
/// Three-mode strategy:
///
/// **While moving** — client is ground truth. No correction at all. The server
/// is always 100ms+ behind due to tick rate; correcting during movement creates
/// visible jitter.
///
/// **Just stopped (< IDLE_SETTLE_SECS)** — do nothing. The server is still
/// processing the stop and its position hasn't stabilised yet. Correcting now
/// would snap the player back before the server arrives.
///
/// **Settled idle (>= IDLE_SETTLE_SECS)** — gently lerp toward server at
/// IDLE_LERP_FACTOR. Corrects any accumulated tick-rate drift (typically
/// 0.5–2 tiles) without visible artifact. No cap on error — larger drifts
/// converge more slowly but always converge.
///
/// **Hard snap** — fires in all modes when error exceeds 3 tiles (genuine desync).
pub fn reconcile_local_player_to_server(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<InputBindings>,
    mut idle_secs: Local<f32>,
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

    // Wait for a real server position before committing. The server sends
    // (0, 0) for the first several snapshots while it loads the character from
    // the database. We treat any position within 1wu of origin as "not yet
    // loaded" and skip reconciliation entirely until the real position arrives.
    if !status.initial_sync_done {
        if target.length_squared() < 1.0 {
            // Server hasn't loaded the character yet — don't move the player.
            return;
        }
        // Real position received — snap silently. No warning; this is expected.
        logical.0 = target;
        xform.translation.x = target.x;
        xform.translation.z = target.y;
        status.initial_sync_done = true;
        *idle_secs = 0.0;
        info!(
            "reconcile initial-sync: placed at ({:.1}, {:.1})",
            target.x, target.y,
        );
        return;
    }

    // Hard snap fires for genuine desyncs regardless of movement state.
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

    let is_moving =
        keyboard.pressed(bindings.move_forward) ||
        keyboard.pressed(bindings.move_back)    ||
        keyboard.pressed(bindings.move_left)    ||
        keyboard.pressed(bindings.move_right);

    if is_moving {
        // Moving — trust the client completely, reset the settle timer.
        *idle_secs = 0.0;
        return;
    }

    // Accumulate idle time before applying any correction.
    *idle_secs += time.delta_secs();
    if *idle_secs < IDLE_SETTLE_SECS {
        return;
    }

    // Settled idle — converge toward server position only if the drift is
    // large enough to be meaningful. Sub-tile differences (< half a tile) are
    // accepted as-is; the player can rest on any position inside a tile without
    // being nudged toward the server's resting point.
    if error > IDLE_CORRECT_MIN_ERROR {
        let new_pos = local.lerp(target, IDLE_LERP_FACTOR);
        logical.0 = new_pos;
        xform.translation.x = new_pos.x;
        xform.translation.z = new_pos.y;
    }
}
