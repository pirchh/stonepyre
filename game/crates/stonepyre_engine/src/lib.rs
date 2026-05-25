use bevy::app::AnimationSystems;
use bevy::ecs::schedule::common_conditions::resource_exists;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub mod plugins;

/// Engine systems that should only run once we're actually in-world.
/// The app crate gates this set with `run_if(in_state(AppMode::InWorld))`.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum EngineSet {
    Runtime,
}

pub struct StonepyreEnginePlugin;

impl Plugin for StonepyreEnginePlugin {
    fn build(&self, app: &mut App) {
        let content = stonepyre_content::default_content_db();

        app
            // ------------------------------------------------------------
            // REQUIRED RESOURCES (exist before any Update systems)
            // ------------------------------------------------------------
            .insert_resource(plugins::ui::ContextMenuState::default())
            .insert_resource(plugins::interaction::ServerAuthoritativeInteractions::default())
            .insert_resource(plugins::interaction::WorldInteractionBlocker::default())
            .insert_resource(stonepyre_world::WorldGrid::new(
                64,
                Box::new(stonepyre_world::FlatWorldSource::new(1337, 0)),
            ))
            // ✅ Input bindings (configurable later via settings)
            .insert_resource(plugins::input::InputBindings::default())
            // ✅ Camera rig — zoom and pitch, updated by scroll wheel
            .insert_resource(plugins::world::CameraRig::default())
            // ✅ Nearest interactable within range (updated every frame)
            .insert_resource(plugins::world::NearbyInteractable::default())
            // ✅ Content-owned defs loaded into engine resources:
            .insert_resource(plugins::inventory::ItemDb(content.items.clone()))
            .insert_resource(plugins::inventory::ContainerDb(content.containers.clone()))
            // ✅ Animation resources — default until the world spawns and populates them:
            .insert_resource(plugins::animation::PlayerGltfHandle::default())
            .insert_resource(plugins::animation::PlayerAnimGraph::default())
            // ✅ Harvest definitions come from content
            .insert_resource(plugins::skills::HarvestDb::from_defs(content.harvest.clone()))
            .insert_resource(plugins::rng::GameRng::default())
            // ------------------------------------------------------------
            // Messages MUST be initialized before any MessageReader/Writer
            // ------------------------------------------------------------
            .add_message::<plugins::input::ClickMsg>()
            .add_message::<plugins::ui::MenuSelectMsg>()
            .add_message::<plugins::interaction::IntentMsg>()
            .add_message::<plugins::interaction::ActionResolvedMsg>()
            .add_message::<plugins::progression::xp::GainXpMsg>();

        // ❌ IMPORTANT: DO NOT spawn world/camera on Startup anymore.
        // World spawn is triggered by the app on entering AppMode::InWorld.

        // ✅ All engine runtime systems live in EngineSet::Runtime.
        // The app crate will gate EngineSet::Runtime with:
        //   app.configure_sets(Update, EngineSet::Runtime.run_if(in_state(AppMode::InWorld)));

        // World + input + interaction systems (batch 1 of 2).
        app.add_systems(
            Update,
            (
                // ---- World maintenance ----
                plugins::world::sync_world_grid_blocked,
                plugins::world::camera_follow_player,
                plugins::world::camera_zoom_and_pitch,
                // ---- Animation graph setup (runs until GLB is loaded & linked) ----
                plugins::animation::setup_player_anim_graph,
                plugins::animation::link_anim_player_to_player
                    .after(plugins::animation::setup_player_anim_graph),
                // ---- Proximity detection ----
                plugins::interaction::update_nearby_interactable,
                // ---- Input + context menu + interaction intent planning ----
                (
                    plugins::input::emit_click_messages,
                    plugins::interaction::handle_interact_key,
                    plugins::ui::context_menu_overlay_system,
                    plugins::ui::handle_context_menu_overlay_clicks,
                    plugins::interaction::handle_clicks_build_candidates,
                    plugins::interaction::handle_menu_selection_emit_intent,
                    plugins::ui::clear_context_menu_consumed_click,
                    plugins::interaction::plan_intents_to_actions,
                )
                    .chain(),
            )
                .in_set(EngineSet::Runtime),
        );

        // Movement + skills + progression systems (batch 2 of 2).
        app.add_systems(
            Update,
            (
                // ---- Action execution/resolution ----
                (
                    plugins::interaction::advance_action_to_impact_when_ready,
                    plugins::interaction::drive_action_clip_on_impact,
                    plugins::interaction::resolve_actions_on_impact,
                )
                    .chain(),
                plugins::interaction::debug_print_resolved_actions,
                // ---- Movement + Animation ----
                plugins::movement::wasd_movement,
                plugins::animation::animate_humanoid
                    .after(plugins::movement::wasd_movement)
                    .after(plugins::animation::link_anim_player_to_player),
                // ---- Harvest regen + visibility sync (generic) ----
                plugins::skills::tick_harvest_regen
                    .run_if(resource_exists::<plugins::skills::HarvestDb>),
                plugins::skills::sync_harvest_node_visibility
                    .run_if(resource_exists::<plugins::skills::HarvestDb>),
                // ---- Skill handlers ----
                plugins::skills::woodcutting::on_action_resolved_apply_woodcutting
                    .run_if(resource_exists::<plugins::skills::HarvestDb>),
                // ---- Progression ----
                plugins::progression::xp::apply_xp_system,
            )
                .in_set(EngineSet::Runtime),
        );

        // Re-apply the movement system's logical XZ position after Bevy's animation
        // system (PostUpdate) has potentially overwritten it with root-motion data.
        // Must run after animation and before transform propagation so children
        // inherit the corrected world position in the same frame.
        app.add_systems(
            PostUpdate,
            plugins::world::apply_logical_pos
                .after(AnimationSystems)
                .before(TransformSystems::Propagate),
        );
    }
}
