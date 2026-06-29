use bevy::prelude::*;

use stonepyre_world::WorldGrid;

use crate::plugins::input::InputBindings;
use crate::plugins::world::{Facing, LogicalPos2d, MoveSpeed, Player};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Set every frame by `wasd_movement` so other systems (animation, networking)
/// can tell whether the player is currently walking without re-reading input.
#[derive(Component, Default)]
pub struct IsWalking(pub bool);

/// Legacy tile-step component kept so the reconciliation code compiles.
/// No longer driven by the player movement system.
#[derive(Component, Clone, Copy, Debug)]
pub struct StepTo(pub stonepyre_world::TilePos);

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

pub fn wasd_movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<InputBindings>,
    world: Option<Res<WorldGrid>>,
    mut q: Query<(
        &mut Transform,
        &mut LogicalPos2d,
        &MoveSpeed,
        &mut Facing,
        &mut IsWalking,
    ), With<Player>>,
) {
    let Ok((mut xform, mut logical, speed, mut facing, mut is_walking)) = q.single_mut() else {
        return;
    };

    // --- 8-directional input from bound keys ---
    // Vec2 convention: x = world X, y = world Z (matches LogicalPos2d)
    let mut dir = Vec2::ZERO;
    if keyboard.pressed(bindings.move_forward) { dir.y -= 1.0; }
    if keyboard.pressed(bindings.move_back)    { dir.y += 1.0; }
    if keyboard.pressed(bindings.move_left)    { dir.x -= 1.0; }
    if keyboard.pressed(bindings.move_right)   { dir.x += 1.0; }

    is_walking.0 = dir != Vec2::ZERO;

    if dir == Vec2::ZERO {
        return;
    }

    let dir = dir.normalize();

    // Update facing from dominant movement axis.
    *facing = facing_from_dir(dir);

    let delta = dir * speed.0 * time.delta_secs();

    let new_pos = if let Some(world) = &world {
        try_move(logical.0, delta, world)
    } else {
        logical.0 + delta
    };

    logical.0 = new_pos;
    xform.translation.x = new_pos.x;
    xform.translation.z = new_pos.y; // Vec2.y stores world-Z
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try the full move, then slide along X, then Z on collision. Delegates to the
/// shared `stonepyre_world::slide_move` so the client predictor integrates
/// movement identically to the server simulation (required for replay).
fn try_move(pos: Vec2, delta: Vec2, world: &WorldGrid) -> Vec2 {
    let out = stonepyre_world::slide_move(
        [pos.x, pos.y],
        [delta.x, delta.y],
        |t| world.is_blocked(t),
    );
    Vec2::new(out[0], out[1])
}

pub fn facing_from_dir(dir: Vec2) -> Facing {
    // If both axes are meaningful (diagonal), return an intercardinal direction.
    // Threshold of 0.35 cleanly separates diagonal from cardinal for a
    // normalised 2-D vector (pure diagonal ≈ 0.707 on each axis).
    let diagonal = dir.x.abs() > 0.35 && dir.y.abs() > 0.35;
    if diagonal {
        match (dir.x > 0.0, dir.y < 0.0) {
            (true,  true)  => Facing::NorthEast,
            (false, true)  => Facing::NorthWest,
            (true,  false) => Facing::SouthEast,
            (false, false) => Facing::SouthWest,
        }
    } else if dir.x.abs() >= dir.y.abs() {
        if dir.x > 0.0 { Facing::East } else { Facing::West }
    } else {
        if dir.y < 0.0 { Facing::North } else { Facing::South }
    }
}
