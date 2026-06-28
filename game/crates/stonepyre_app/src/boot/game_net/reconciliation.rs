use bevy::prelude::*;

use stonepyre_engine::plugins::input::InputBindings;
use stonepyre_engine::plugins::world::{LogicalPos2d, MoveSpeed, Player};
use stonepyre_world::{slide_move, world3d_to_tile, WorldGrid, TILE_SIZE};

use super::status::GameNetStatus;

/// Beyond this the local view snaps instantly (genuine desync / teleport).
const HARD_SNAP_THRESHOLD: f32 = TILE_SIZE * 3.0;

/// How far behind "now" the server's authoritative position is, roughly: one
/// snapshot interval plus transit. We project the authoritative position forward
/// by the held input over this window so we reconcile against where the server
/// *will* be, not where it was — otherwise pure latency reads as error and the
/// player rubber-bands. Tunable; ~0.12s suits a 10Hz snapshot rate on LAN.
const SERVER_LAG_SECS: f32 = 0.12;

/// Ignore errors smaller than this — avoids visible jitter and lets the player
/// rest anywhere within ~a fifth of a tile of the server's resting point.
const CORRECT_DEADZONE: f32 = TILE_SIZE * 0.2;

/// Per-frame lerp toward the reconciled target: gentle while moving (a small
/// error vanishes over a few frames, invisibly), a touch firmer while idle to
/// settle onto the authoritative resting position.
const MOVING_CORRECT_RATE: f32 = 0.15;
const IDLE_CORRECT_RATE: f32 = 0.30;

/// Reconcile the locally-predicted player against the server's authoritative
/// position by replaying the held input forward.
///
/// The client predicts movement locally every frame (engine `wasd_movement`).
/// Each tick the server sends its authoritative position plus `last_input_seq`
/// (the last input it applied). Every frame, here:
///
/// 1. Take the authoritative position and — when the server is current on our
///    input — re-integrate the held direction forward through the shared,
///    collision-aware `slide_move` for `SERVER_LAG_SECS`. For this held-direction
///    model, "replay the unacknowledged input" *is* this forward integration: the
///    server is mid-applying that direction, so we project it to ~now. If the
///    server hasn't applied our latest turn yet (its echoed seq is behind), its
///    position still reflects the old direction, so we skip the feed-forward.
/// 2. Continuously, gently lerp the rendered position toward that target — while
///    moving *and* idle. Genuine error (a wall we mispredicted, a server clamp)
///    converges away smoothly; pure latency contributes ~nothing because the
///    feed-forward already accounts for it. This runs *before* `wasd_movement`,
///    which then applies this frame's input on top (and re-clamps collisions).
/// 3. Hard-snap only on a multi-tile gap (real desync / teleport).
///
/// Replaces the old "client is ground truth while moving; settle-then-lerp when
/// idle; snap at 3 tiles" heuristic, which let error accumulate invisibly and
/// then teleport the player on stop.
pub fn reconcile_local_player_to_server(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<InputBindings>,
    world: Option<Res<WorldGrid>>,
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<(&mut Transform, &mut LogicalPos2d, &MoveSpeed), With<Player>>,
) {
    let Some(server_pos) = status.server_pos else {
        return;
    };

    let Ok((mut xform, mut logical, speed)) = player_q.single_mut() else {
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

    // Held input direction (same axes/convention as engine `wasd_movement`).
    let mut dir = Vec2::ZERO;
    if keyboard.pressed(bindings.move_forward) { dir.y -= 1.0; }
    if keyboard.pressed(bindings.move_back)    { dir.y += 1.0; }
    if keyboard.pressed(bindings.move_left)    { dir.x -= 1.0; }
    if keyboard.pressed(bindings.move_right)   { dir.x += 1.0; }
    let moving = dir != Vec2::ZERO;

    // Has the server applied our latest input? Find our own player in the latest
    // snapshot; if its echoed seq is behind, its position still reflects the
    // previous direction, so skip the feed-forward for ~one RTT after a turn.
    let server_current = {
        let pid = status.player_id;
        let acked = pid
            .and_then(|pid| status.latest_players.iter().find(|p| p.player_id == pid))
            .map(|p| p.last_input_seq);
        match acked {
            Some(seq) => seq >= status.last_sent_input_seq,
            None => true,
        }
    };

    // Feed-forward: project the authoritative position forward by the held input
    // to ~now, collision-aware, so we reconcile against the server's *current*
    // position rather than its ~one-tick-stale snapshot.
    let target = if moving && server_current {
        let d = dir.normalize();
        let delta = [d.x * speed.0 * SERVER_LAG_SECS, d.y * speed.0 * SERVER_LAG_SECS];
        let out = match world.as_ref() {
            Some(w) => slide_move([auth.x, auth.y], delta, |t| w.is_blocked(t)),
            None => [auth.x + delta[0], auth.y + delta[1]],
        };
        Vec2::new(out[0], out[1])
    } else {
        auth
    };

    let local = logical.0;
    let error = (target - local).length();
    status.drift_tiles = Some((error / TILE_SIZE).round() as i32);
    status.local_tile = Some(world3d_to_tile(xform.translation));

    // Hard snap for genuine desyncs / teleports, regardless of movement state.
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

    // Continuous smooth correction toward the (feed-forward) authoritative target.
    if error > CORRECT_DEADZONE {
        let rate = if moving { MOVING_CORRECT_RATE } else { IDLE_CORRECT_RATE };
        let new_pos = local.lerp(target, rate);
        logical.0 = new_pos;
        xform.translation.x = new_pos.x;
        xform.translation.z = new_pos.y;
    }
}
