use bevy::prelude::*;

pub mod camera;
pub mod fonts;
pub mod net;
pub mod ui;

// ✅ must be public so main.rs can use Screen + BootState
pub mod state;
pub use state::{BootState, Screen};

pub struct BootFlowPlugin;

impl Plugin for BootFlowPlugin {
    fn build(&self, app: &mut App) {
        app
            // State + resources
            .init_state::<Screen>()
            .init_resource::<BootState>()
            .init_resource::<net::NetRuntime>()
            .init_resource::<fonts::UiFonts>()
            // Startup init
            .add_systems(
                Startup,
                (
                    camera::spawn_boot_ui_camera,
                    net::init_server_base_url,
                    fonts::load_ui_fonts,
                ),
            )
            // Net pump
            .add_systems(Update, net::pump_net_results)
            // Screen lifecycle
            .add_systems(OnExit(Screen::MainMenu), ui::despawn_screen)
            .add_systems(OnExit(Screen::AccountLogin), ui::despawn_screen)
            .add_systems(OnExit(Screen::CharacterSelect), ui::despawn_screen)
            // When entering the real game, remove the boot camera
            .add_systems(OnEnter(Screen::InWorld), camera::despawn_boot_ui_camera)
            // Enters
            .add_systems(OnEnter(Screen::MainMenu), ui::main_menu_enter)
            .add_systems(OnEnter(Screen::AccountLogin), ui::login_enter)
            .add_systems(OnEnter(Screen::CharacterSelect), ui::character_select_enter)
            // Updates (gated by state)
            .add_systems(Update, ui::main_menu_update.run_if(in_state(Screen::MainMenu)))
            .add_systems(Update, ui::login_update.run_if(in_state(Screen::AccountLogin)))
            .add_systems(
                Update,
                ui::character_select_update.run_if(in_state(Screen::CharacterSelect)),
            );
    }
}