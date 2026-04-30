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
        .add_systems(
            OnEnter(Screen::InWorld),
            (
                enable_game_ui_on_enter_world,
                boot::game_net::spawn_game_net_overlay,
                start_world_on_enter,
            ),
        )
        .add_systems(
            Update,
            (
                boot::game_net::pump_game_net_results,
                boot::game_net::sync_inventory_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::send_walk_intents_to_server_runtime
                    .after(stonepyre_engine::plugins::interaction::plan_intents_to_actions)
                    .before(stonepyre_engine::plugins::movement::follow_path_to_next_tile),
                boot::game_net::reconcile_local_player_to_server
                    .after(boot::game_net::pump_game_net_results)
                    .after(boot::game_net::send_walk_intents_to_server_runtime)
                    .before(stonepyre_engine::plugins::movement::follow_path_to_next_tile),
                boot::game_net::play_server_authoritative_action_visuals
                    .after(boot::game_net::reconcile_local_player_to_server)
                    .after(stonepyre_engine::plugins::movement::follow_path_to_next_tile)
                    .before(stonepyre_engine::plugins::animation::animate_humanoid),
                boot::game_net::sync_network_target_marker_from_last_move
                    .after(stonepyre_engine::plugins::world::debug_draw_target_marker)
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::sync_remote_players_from_snapshots,
                boot::game_net::animate_remote_players_from_snapshots,
                boot::game_net::update_game_net_overlay,
            )
                .run_if(in_state(Screen::InWorld)),
        )
        .add_systems(
            OnExit(Screen::InWorld),
            (
                disable_game_ui_on_exit_world,
                boot::game_net::despawn_game_net_overlay,
                boot::game_net::despawn_remote_players,
            ),
        )
        .run();
}

fn enable_game_ui_on_enter_world(mut enabled: ResMut<stonepyre_ui::GameUiEnabled>) {
    enabled.0 = true;
}

fn disable_game_ui_on_exit_world(
    mut commands: Commands,
    mut enabled: ResMut<stonepyre_ui::GameUiEnabled>,
) {
    enabled.0 = false;
    commands.insert_resource(
        stonepyre_engine::plugins::interaction::ServerAuthoritativeInteractions(false),
    );
}

fn start_world_on_enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    harvest_defs: Option<Res<stonepyre_engine::plugins::skills::HarvestDb>>,
    mut boot: ResMut<BootState>,
    mut game_net: ResMut<boot::game_net::GameNetRuntime>,
    mut game_status: ResMut<boot::game_net::GameNetStatus>,
) {
    let character_id = boot.pending_start_world.take().unwrap_or(Uuid::nil());
    let has_session = boot.session.is_some();

    commands.insert_resource(
        stonepyre_engine::plugins::interaction::ServerAuthoritativeInteractions(has_session),
    );

    if let Some(session) = boot.session.as_ref() {
        boot::game_net::spawn_game_ws(
            &mut game_net,
            &mut game_status,
            boot.server_base_url.clone(),
            session.token.clone(),
            character_id,
        );
    } else {
        warn!("entering world without a session; skipping game websocket join");
    }

    stonepyre_engine::plugins::world::spawn_demo_world_for_character(
        &mut commands,
        &asset_server,
        harvest_defs.as_deref(),
        character_id,
    );
}
