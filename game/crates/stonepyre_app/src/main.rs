// crates/stonepyre_app/src/main.rs
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use uuid::Uuid;

mod boot;

use boot::{BootState, Screen};

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let default_root = format!("{}/../../assets", manifest_dir);
    let asset_root = std::env::var("STONEPYRE_ASSET_ROOT").unwrap_or(default_root);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Stonepyre".to_string(),
                        resolution: (1920, 1080).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(boot::BootFlowPlugin)
        .add_plugins(stonepyre_engine::StonepyreEnginePlugin)
        .add_plugins(stonepyre_ui::StonepyreUiPlugin)
        // ✅ World entry: enable in-world UI + spawn world
        .add_systems(
            OnEnter(Screen::InWorld),
            (
                enable_game_ui_on_enter_world,
                start_world_on_enter,
            ),
        )
        // ✅ Leaving world: turn off in-world UI so MainMenu is clean/fullscreen
        .add_systems(OnExit(Screen::InWorld), disable_game_ui_on_exit_world)
        .run();
}

fn enable_game_ui_on_enter_world(mut enabled: ResMut<stonepyre_ui::GameUiEnabled>) {
    enabled.0 = true;
}

fn disable_game_ui_on_exit_world(mut enabled: ResMut<stonepyre_ui::GameUiEnabled>) {
    enabled.0 = false;
}

fn start_world_on_enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    harvest_defs: Option<Res<stonepyre_engine::plugins::skills::HarvestDb>>,
    mut boot: ResMut<BootState>,
) {
    let character_id = boot.pending_start_world.take().unwrap_or(Uuid::nil());

    stonepyre_engine::plugins::world::spawn_demo_world_for_character(
        &mut commands,
        &asset_server,
        harvest_defs.as_deref(),
        character_id,
    );
}