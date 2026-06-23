// crates/stonepyre_ui/src/lib.rs
use bevy::prelude::*;

use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::input::emit_click_messages;

pub mod bag;
pub mod bank;
pub mod character_state;
pub mod character_tab;
pub mod config;
pub mod debug_grant;
pub mod drag;
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

fn clear_world_interaction_blocker_system(mut blocker: ResMut<WorldInteractionBlocker>) {
    blocker.0 = false;
}

pub struct StonepyreUiPlugin;

impl Plugin for StonepyreUiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Global toggle driven by stonepyre_app boot screens
            .insert_resource(GameUiEnabled::default())

            // Configurable keybinds (later: settings menu edits this)
            .insert_resource(config::UiBindings::default())

            // Reset UI click blocking once per UI frame. Individual panels then opt back in
            // when the cursor is actually over them. This prevents stale blockers from
            // making the world randomly stop accepting clicks.
            // Runs in PreUpdate so it always clears before any Update system runs.
            .add_systems(PreUpdate, clear_world_interaction_blocker_system.run_if(game_ui_enabled))

            // HUD
            .insert_resource(hud::HudState::default())
            // ✅ HUD is now “presence-managed”: spawn when enabled, despawn when disabled.
            .add_systems(Update, hud::ensure_hud_bar_system)

            // Only run HUD interactions/tooltips/keys when enabled
            .add_systems(Update, hud::hud_interactions_system.run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_active_tab_highlight_system.run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_world_interaction_blocker_system
                .before(emit_click_messages)
                .run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_tooltip_system.run_if(game_ui_enabled))
            .add_systems(Update, hud::hud_keyboard_toggles.run_if(game_ui_enabled))

            // Drag-and-drop (must run before inventory/bag context menus so click
            // resolution happens before those systems clear their Interaction state)
            .insert_resource(drag::DragState::default())
            .add_systems(
                Update,
                (
                    drag::drag_begin_system,
                    drag::drag_update_system,
                    drag::drag_end_system,
                )
                    .chain()
                    .run_if(game_ui_enabled),
            )

            // Inventory panel (render-only; HUD controls open/close)
            .insert_resource(inventory::InventoryUiState::default())
            .insert_resource(inventory::InventoryItemActionQueue::default())
            .add_systems(
                Update,
                (
                    inventory::inventory_panel_sync_system
                        .before(emit_click_messages),
                    inventory::inventory_item_context_menu_system,
                )
                    .run_if(game_ui_enabled),
            )

            // Character panel (render-only; HUD controls open/close)
            .insert_resource(character_state::CharacterUiState::default())
            .insert_resource(character_tab::CharacterEquipActionQueue::default())
            .add_systems(
                Update,
                (
                    character_tab::character_tab_panel_sync_system
                        .before(emit_click_messages),
                    character_tab::character_bag_context_menu_system,
                )
                    .run_if(game_ui_enabled),
            )

            // Bag panel (opens when bag slot button is clicked in character panel)
            .insert_resource(bag::BagUiState::default())
            .insert_resource(bag::BagItemActionQueue::default())
            .add_systems(
                Update,
                (
                    bag::bag_panel_sync_system
                        .before(emit_click_messages),
                    bag::bag_context_menu_system,
                )
                    .run_if(game_ui_enabled),
            )

            // Bank panel (opens when the player interacts with a bank booth)
            .insert_resource(bank::BankUiState::default())
            .insert_resource(bank::BankItemActionQueue::default())
            .add_systems(
                Update,
                (
                    bank::bank_panel_sync_system
                        .before(emit_click_messages),
                    bank::bank_interaction_system,
                )
                    .run_if(game_ui_enabled),
            )

            // Debug item grant panel (admin only, toggled by F2)
            .insert_resource(debug_grant::IsAdminAccount::default())
            .insert_resource(debug_grant::DebugGrantUiState::default())
            .insert_resource(debug_grant::DebugGrantActionQueue::default())
            .add_systems(
                Update,
                (
                    debug_grant::debug_grant_toggle_system,
                    debug_grant::debug_grant_panel_sync_system,
                )
                    .run_if(game_ui_enabled),
            );
    }
}
