use bevy::prelude::*;

/// UI-level configurable keybinds.
/// Later: settings menu will edit this resource.
#[derive(Resource, Clone, Debug)]
pub struct UiBindings {
    pub toggle_inventory: KeyCode,
    pub toggle_character: KeyCode,
    pub toggle_debug_grant: KeyCode,
}

impl Default for UiBindings {
    fn default() -> Self {
        Self {
            toggle_inventory: KeyCode::KeyI,
            toggle_character: KeyCode::KeyC,
            toggle_debug_grant: KeyCode::F2,
        }
    }
}