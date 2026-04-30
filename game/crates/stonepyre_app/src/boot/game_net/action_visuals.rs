use bevy::prelude::*;

use stonepyre_engine::plugins::skills::{AnimClip, RequestedAnim, RequestedAnimMode};
use stonepyre_engine::plugins::world::{player_feet_world, Facing, Player};
use stonepyre_world::{world_to_tile, TilePos};

use super::protocol::{ActionState, InteractionAction, InteractionTarget};
use super::status::GameNetStatus;

const CHOP_VISUAL_SECS: f32 = 0.75;

/// Client-side presentation for server-owned actions.
///
/// Gameplay state is owned by the server. This system only mirrors the latest
/// authoritative action lifecycle from snapshots into animation. If the server
/// action is not Active, the skill clip is removed immediately.
pub fn play_server_authoritative_action_visuals(
    mut commands: Commands,
    mut player_q: Query<(
        Entity,
        &Transform,
        &mut Facing,
        Option<&RequestedAnim>,
    ), With<Player>>,
    status: Res<GameNetStatus>,
) {
    let Ok((player_ent, player_xform, mut facing, requested_anim)) = player_q.single_mut() else {
        return;
    };

    let Some(action) = status.server_action.as_ref() else {
        if requested_anim.is_some() {
            commands.entity(player_ent).remove::<RequestedAnim>();
        }
        return;
    };

    if action.action != InteractionAction::ChopDown || action.state != ActionState::Active {
        if requested_anim.is_some() {
            commands.entity(player_ent).remove::<RequestedAnim>();
        }
        return;
    }

    let InteractionTarget::Tile(target) = action.target.clone();

    let server_tile = status
        .server_tile
        .unwrap_or_else(|| world_to_tile(player_feet_world(player_xform)));
    let local_tile = world_to_tile(player_feet_world(player_xform));

    // Safety guard: presentation should never continue if both local and server
    // positions are out of action range. The server should cancel first, but this
    // prevents a stale snapshot/frame from leaving the clip playing.
    if manhattan(server_tile, target) > 1 && manhattan(local_tile, target) > 1 {
        if requested_anim.is_some() {
            commands.entity(player_ent).remove::<RequestedAnim>();
        }
        return;
    }

    *facing = facing_toward(server_tile, target, *facing);

    if requested_anim.is_none() {
        commands.entity(player_ent).insert(RequestedAnim {
            clip: AnimClip::Woodcutting,
            mode: RequestedAnimMode::Loop {
                timer: Timer::from_seconds(CHOP_VISUAL_SECS, TimerMode::Repeating),
            },
        });
    }
}

fn manhattan(a: TilePos, b: TilePos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn facing_toward(from: TilePos, to: TilePos, current: Facing) -> Facing {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    if dx == 0 && dy == 0 {
        return current;
    }

    if dx.abs() >= dy.abs() {
        if dx > 0 {
            Facing::East
        } else {
            Facing::West
        }
    } else if dy > 0 {
        Facing::North
    } else {
        Facing::South
    }
}
