use bevy::prelude::*;

/// All configurable key bindings for the game.
/// Systems read from this resource instead of hardcoding `KeyCode` values,
/// so the player can remap keys without a code change.
#[derive(Resource, Clone, Debug)]
pub struct InputBindings {
    // Movement
    pub move_forward: KeyCode,
    pub move_back: KeyCode,
    pub move_left: KeyCode,
    pub move_right: KeyCode,

    // Interaction
    pub interact: KeyCode,

    /// Context-sensitive action (attack/harvest/use) — Project Zomboid style.
    pub action: KeyCode,

    // UI
    pub inventory_toggle: KeyCode,
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            move_forward: KeyCode::KeyW,
            move_back: KeyCode::KeyS,
            move_left: KeyCode::KeyA,
            move_right: KeyCode::KeyD,
            interact: KeyCode::KeyE,
            action: KeyCode::Space,
            inventory_toggle: KeyCode::KeyI,
        }
    }
}
