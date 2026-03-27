use bevy::prelude::*;

#[derive(Resource, Clone, Debug)]
pub struct InputBindings {
    pub inventory_toggle: KeyCode,
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            inventory_toggle: KeyCode::KeyI,
        }
    }
}