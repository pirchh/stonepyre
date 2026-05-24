use bevy::prelude::*;

use stonepyre_world::{tile_to_world3d, world3d_to_tile, TilePos};

use crate::plugins::world::{Facing, MoveSpeed, Player, TilePath, ARRIVE_EPS};

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
        let from = world3d_to_tile(xform.translation);
        *facing = facing_from_step(from, next);

        commands.entity(ent).insert(StepTo(next));
        next
    };

    // Move toward the 3D world center of the target tile (XZ plane, Y = 0).
    let target = tile_to_world3d(step_tile);
    let cur = Vec2::new(xform.translation.x, xform.translation.z);
    let tgt = Vec2::new(target.x, target.z);

    let to = tgt - cur;
    let dist = to.length();

    if dist <= ARRIVE_EPS {
        // Snap to target and complete the step.
        xform.translation.x = target.x;
        xform.translation.y = 0.0;
        xform.translation.z = target.z;

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

    // Move smoothly toward target in the XZ plane.
    let dir = to / dist.max(0.0001);
    let step = speed.0 * time.delta_secs();
    let delta = dir * step.min(dist);

    xform.translation.x += delta.x;
    xform.translation.z += delta.y; // delta.y here is the Z-axis component
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