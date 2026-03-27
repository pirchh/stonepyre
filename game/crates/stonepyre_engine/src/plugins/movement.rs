use bevy::prelude::*;

use stonepyre_world::{tile_to_world_center, world_to_tile, TilePos};

use crate::plugins::world::{
    player_feet_world, Facing, MoveSpeed, Player, TilePath, ARRIVE_EPS, FOOT_OFFSET_Y,
};

/// Indicates the player is currently moving toward this tile.
/// This stays present during in-between frames so animation can reliably detect “walking”.
#[derive(Component, Clone, Copy, Debug)]
pub struct StepTo(pub TilePos);

pub fn follow_path_to_next_tile(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        &mut Transform,
        &MoveSpeed,
        &mut TilePath,
        &mut Facing,
        Option<&StepTo>,
    ), With<Player>>,
) {
    let Ok((ent, mut xform, speed, mut path, mut facing, step_opt)) = q.single_mut() else {
        return;
    };

    // If we are not currently stepping, pick the next tile (but DO NOT pop yet).
    let step_tile = if let Some(step) = step_opt {
        step.0
    } else {
        let Some(next) = path.tiles.front().copied() else {
            return;
        };

        // Update facing immediately based on next tile.
        let from = world_to_tile(player_feet_world(&xform));
        *facing = facing_from_step(from, next);

        commands.entity(ent).insert(StepTo(next));
        next
    };

    // Move toward the center of the step tile (feet-based).
    let target_center = tile_to_world_center(step_tile);
    let target_feet = Vec2::new(target_center.x, target_center.y);
    let cur_feet = player_feet_world(&xform);

    let to = target_feet - cur_feet;
    let dist = to.length();

    if dist <= ARRIVE_EPS {
        // Snap to target and complete the step.
        xform.translation.x = target_feet.x;
        xform.translation.y = target_feet.y + FOOT_OFFSET_Y;

        // Now pop the tile we just reached.
        if let Some(front) = path.tiles.front().copied() {
            if front == step_tile {
                path.tiles.pop_front();
            }
        }

        // Clear stepping state; next tick will pick the next tile if available.
        commands.entity(ent).remove::<StepTo>();
        return;
    }

    // Move smoothly toward target.
    let dir = to / dist.max(0.0001);
    let step = speed.0 * time.delta_secs();
    let delta = dir * step.min(dist);

    xform.translation.x += delta.x;
    xform.translation.y += delta.y;
}

/// Same logic as interaction.rs helper, duplicated locally to avoid circular imports.
fn facing_from_step(from: TilePos, to: TilePos) -> Facing {
    if to.x > from.x {
        Facing::East
    } else if to.x < from.x {
        Facing::West
    } else if to.y > from.y {
        Facing::North
    } else {
        Facing::South
    }
}