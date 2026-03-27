// crates/stonepyre_ui/src/lib.rs
use bevy::prelude::*;

pub mod character;
pub mod config;
pub mod hud;
pub mod inventory;

/// When false: in-world UI (HUD, inventory, character panels) must not exist.
#[derive(Resource, Debug, Clone, Copy)]
pub struct GameUiEnabled(pub bool);

impl Default for GameUiEnabled {
    fn default() -> Self {
        Self(false) // ✅ start disabled because the app starts in MainMenu
    }
}

fn game_ui_enabled(enabled: Res<GameUiEnabled>) -> bool {
    enabled.0
}

pub struct StonepyreUiPlugin;

impl Plugin for StonepyreUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Global toggle driven by stonepyre_app boot screens
            .insert_resource(GameUiEnabled::default())

            // Configurable keybinds (later: settings menu edits this)
            .insert_resource(config::UiBindings::default())

            // HUD
            .insert_resource(hud::HudState::default())
            // ✅ HUD is now “presence-managed”: spawn when enabled, despawn when disabled.
            .add_systems(Update, hud::ensure_hud_bar_system)

            // Only run HUD interactions/tooltips/keys when enabled
            .add_systems(Update, hud::hud_interactions_system.run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_tooltip_system.run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_keyboard_toggles.run_if(game_ui_enabled))

            // Inventory panel (render-only; HUD controls open/close)
            .insert_resource(inventory::InventoryUiState::default())
            .add_systems(Update, inventory::inventory_panel_sync_system.run_if(game_ui_enabled))

            // Character panel (render-only; HUD controls open/close)
            .insert_resource(character::CharacterUiState::default())
            .add_systems(Update, character::character_panel_sync_system.run_if(game_ui_enabled))
            .add_systems(Update, character::equip_slot_hover_system.run_if(game_ui_enabled));
    }
}