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
        .insert_resource(boot::game_net::PendingGroundItemPickup::default())
        .insert_resource(boot::game_net::PendingBankOpen::default())
        .insert_resource(stonepyre_engine::plugins::inventory::PlayerBagSlots::default())
        .add_plugins(boot::BootFlowPlugin)
        .add_plugins(stonepyre_engine::StonepyreEnginePlugin)
        .add_plugins(stonepyre_ui::StonepyreUiPlugin)
        .add_systems(
            OnEnter(Screen::InWorld),
            (
                enable_game_ui_on_enter_world,
                boot::game_net::spawn_game_net_overlay,
                boot::game_net::spawn_xp_feedback_layer,
                start_world_on_enter,
            ),
        )
        .add_systems(
            Update,
            (
                boot::game_net::pump_game_net_results,
                boot::game_net::sync_inventory_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::sync_bag_slots_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::send_inventory_item_actions_to_server,
                boot::game_net::send_bag_item_actions_to_server,
                boot::game_net::update_xp_feedback_layer
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::tick_xp_feedback_toasts,
                boot::game_net::sync_harvest_node_visuals_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::sync_ground_item_visuals_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::send_walk_intents_to_server_runtime
                    .after(stonepyre_engine::plugins::interaction::plan_intents_to_actions)
                    .before(stonepyre_engine::plugins::movement::wasd_movement),
                boot::game_net::send_wasd_movement_to_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::process_pending_ground_item_pickups
                    .after(boot::game_net::pump_game_net_results)
                    .after(boot::game_net::reconcile_local_player_to_server),
                boot::game_net::reconcile_local_player_to_server
                    .after(boot::game_net::pump_game_net_results)
                    .after(boot::game_net::send_walk_intents_to_server_runtime)
                    .before(stonepyre_engine::plugins::movement::wasd_movement),
                boot::game_net::play_server_authoritative_action_visuals
                    .after(boot::game_net::reconcile_local_player_to_server)
                    .after(stonepyre_engine::plugins::movement::wasd_movement)
                    .before(stonepyre_engine::plugins::animation::animate_humanoid),
                boot::game_net::sync_remote_players_from_snapshots,
                boot::game_net::animate_remote_players_from_snapshots,
                boot::game_net::update_world_object_depths
                    .after(stonepyre_engine::plugins::movement::wasd_movement)
                    .after(boot::game_net::animate_remote_players_from_snapshots)
                    .after(boot::game_net::sync_harvest_node_visuals_from_server)
                    .after(boot::game_net::sync_ground_item_visuals_from_server),
                boot::game_net::update_game_net_overlay,
                send_debug_grant_actions,
            )
                .run_if(in_state(Screen::InWorld)),
        )
        // Bank sync systems in a separate add_systems to stay within the 20-item tuple limit.
        .add_systems(
            Update,
            (
                boot::game_net::sync_bank_from_server
                    .after(boot::game_net::pump_game_net_results),
                boot::game_net::send_bank_item_actions_to_server,
                boot::game_net::process_pending_bank_open
                    .after(boot::game_net::pump_game_net_results),
            )
                .run_if(in_state(Screen::InWorld)),
        )
        .add_systems(
            OnExit(Screen::InWorld),
            (
                disable_game_ui_on_exit_world,
                boot::game_net::despawn_game_net_overlay,
                boot::game_net::despawn_xp_feedback_layer,
                boot::game_net::despawn_remote_players,
            ),
        )
        .run();
}

fn enable_game_ui_on_enter_world(
    mut enabled: ResMut<stonepyre_ui::GameUiEnabled>,
    mut is_admin: ResMut<stonepyre_ui::debug_grant::IsAdminAccount>,
    boot: Res<BootState>,
) {
    enabled.0 = true;
    is_admin.0 = boot.session.as_ref().map(|s| s.is_admin).unwrap_or(false);
}

fn disable_game_ui_on_exit_world(
    mut commands: Commands,
    mut enabled: ResMut<stonepyre_ui::GameUiEnabled>,
    mut pending_pickup: ResMut<boot::game_net::PendingGroundItemPickup>,
    mut boot: ResMut<BootState>,
) {
    enabled.0 = false;
    pending_pickup.request = None;
    boot.active_character_id = None;
    commands.insert_resource(
        stonepyre_engine::plugins::interaction::ServerAuthoritativeInteractions(false),
    );
}

fn start_world_on_enter(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    harvest_defs: Option<Res<stonepyre_engine::plugins::skills::HarvestDb>>,
    mut boot: ResMut<BootState>,
    mut game_net: ResMut<boot::game_net::GameNetRuntime>,
    mut game_status: ResMut<boot::game_net::GameNetStatus>,
) {
    let character_id = boot.pending_start_world.take().unwrap_or(Uuid::nil());
    boot.active_character_id = Some(character_id);
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
        &mut meshes,
        &mut materials,
        harvest_defs.as_deref(),
        character_id,
    );
}

fn send_debug_grant_actions(
    boot: Res<BootState>,
    net: Res<boot::net::NetRuntime>,
    mut queue: ResMut<stonepyre_ui::debug_grant::DebugGrantActionQueue>,
) {
    let Some(req) = queue.pending_grant.take() else { return };
    let Some(ref session) = boot.session else { return };
    let Some(character_id) = boot.active_character_id else { return };

    boot::net::spawn_admin_grant_item(
        boot.server_base_url.clone(),
        session.token.clone(),
        character_id,
        req.item_id,
        req.quantity,
        net.tx.clone(),
    );
}
