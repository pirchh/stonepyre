use bevy::prelude::*;

use stonepyre_engine::plugins::animation::RequestedAnimStarted;
use stonepyre_engine::plugins::interaction::{AutoChopActive, ServerActionGate};
use stonepyre_engine::plugins::movement::IsWalking;
use stonepyre_engine::plugins::skills::RequestedAnim;
use stonepyre_engine::plugins::world::{player_feet_world, Facing, Player};
use stonepyre_world::{world_to_tile, TilePos};

use super::protocol::{ActionState, InteractionAction, InteractionTarget};
use super::status::GameNetStatus;

/// Drives the woodcutting animation against the server-authoritative action state.
///
/// The server owns the continuous-chop loop: once a Harvest goes Active it stays
/// Active and emits a grant every swing, ending the action only when the node
/// depletes, the player moves out of range, or the inventory fills. So the client
/// is simple:
///
/// - While the action is in-flight: hold `ServerActionGate` (blocks duplicate Space
///   presses) and rotate the player to face the chop target.
/// - On the in-flight falling edge — i.e. ANY terminal state, since the server only
///   sends one when the whole session ends — stop the looping animation.
///
/// The looping `RequestedAnim` itself is inserted by `handle_action_key` on the Space
/// press and animates continuously (native Bevy repeat) until removed here or by
/// `animate_humanoid` the instant the player starts walking.
pub fn play_server_authoritative_action_visuals(
    mut commands: Commands,
    mut prev_was_in_flight: Local<bool>,
    mut server_gate: ResMut<ServerActionGate>,
    mut player_q: Query<(Entity, &Transform, &mut Facing, &IsWalking), With<Player>>,
    status: Res<GameNetStatus>,
) {
    let Ok((player_ent, player_xform, mut facing, is_walking)) = player_q.single_mut() else {
        return;
    };

    let action_in_flight = status.action_event_in_flight;

    // Falling edge = the server ended the harvest session (deplete / out-of-range /
    // inventory-full / movement cancel). Stop the looping swing animation.
    if *prev_was_in_flight && !action_in_flight {
        commands.entity(player_ent).remove::<RequestedAnim>();
        commands.entity(player_ent).remove::<RequestedAnimStarted>();
        commands.entity(player_ent).remove::<AutoChopActive>();
    }

    *prev_was_in_flight = action_in_flight;
    server_gate.0 = action_in_flight;

    // While actively swinging, keep the player turned toward the chop target —
    // but NOT while walking. The server keeps the action Active for ~1 RTT after
    // the player starts moving away, and without this guard action_visuals would
    // override the walk facing, leaving the player moonwalking (moving one way,
    // facing the tree) until the server's cancel arrives.
    let is_active = !is_walking.0
        && status.server_action.as_ref().map_or(false, |a| {
            a.action == InteractionAction::Harvest && a.state == ActionState::Active
        });

    if is_active {
        let Some(action) = status.server_action.as_ref() else {
            return;
        };
        let InteractionTarget::Tile(target) = action.target.clone();
        let server_tile = status
            .server_tile
            .unwrap_or_else(|| world_to_tile(player_feet_world(player_xform)));
        *facing = facing_toward(server_tile, target, *facing);
    }
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
