use bevy::prelude::*;

use stonepyre_engine::plugins::input::InputBindings;
use stonepyre_engine::plugins::interaction::HarvestReadyGate;
use stonepyre_engine::plugins::inventory::{Equipment, ItemDb};
use stonepyre_engine::plugins::skills::HarvestDb;
use stonepyre_engine::plugins::world::{InteractableKind, NearbyInteractable, Player};

use super::harvest_nodes::ServerHarvestNodeVisual;
use super::status::{FeedbackDrop, GameNetStatus};

/// Why (or whether) the player can harvest the nearby node right now.
enum Harvestability {
    Ok,
    /// Not near a harvestable tree, or can't be evaluated locally — don't gate.
    NotApplicable,
    LevelTooLow { required: u32, skill_display: String },
    /// No tool equipped, or the equipped tool's tier is too low.
    NeedTool { tool_name: String },
}

/// Client-side mirror of the server harvest gate, computed each frame for the
/// nearby tree. Drives `HarvestReadyGate` (so `handle_action_key` skips the swing
/// animation for a harvest that would be rejected) and, when the player actually
/// presses the action key on a blocked tree, surfaces a condensed reason as a
/// right-side feedback drop. The server remains authoritative.
pub fn update_harvest_ready_gate(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<InputBindings>,
    nearby: Res<NearbyInteractable>,
    harvest_db: Res<HarvestDb>,
    item_db: Res<ItemDb>,
    mut gate: ResMut<HarvestReadyGate>,
    player_q: Query<&Equipment, With<Player>>,
    node_q: Query<&ServerHarvestNodeVisual>,
    mut status: ResMut<GameNetStatus>,
) {
    let harvestability =
        compute_harvestability(&nearby, &harvest_db, &item_db, &node_q, &player_q, &status);

    gate.0 = matches!(
        harvestability,
        Harvestability::Ok | Harvestability::NotApplicable
    );

    // Only nag when the player actually tries to chop a tree they can't.
    if keyboard.just_pressed(bindings.action) {
        let message = match &harvestability {
            Harvestability::LevelTooLow { required, skill_display } => {
                Some(format!("Need level {required} {skill_display}"))
            }
            Harvestability::NeedTool { tool_name } => Some(format!("Need a {tool_name}")),
            Harvestability::Ok | Harvestability::NotApplicable => None,
        };
        if let Some(text) = message {
            status.feedback_drops.push(FeedbackDrop::Message { text });
        }
    }
}

fn compute_harvestability(
    nearby: &NearbyInteractable,
    harvest_db: &HarvestDb,
    item_db: &ItemDb,
    node_q: &Query<&ServerHarvestNodeVisual>,
    player_q: &Query<&Equipment, With<Player>>,
    status: &GameNetStatus,
) -> Harvestability {
    let Some(entity) = nearby.entity else { return Harvestability::NotApplicable };
    if !matches!(nearby.kind, Some(InteractableKind::Tree)) {
        return Harvestability::NotApplicable;
    }

    // Networked trees carry ServerHarvestNodeVisual (server node id), not the
    // engine HarvestNode component. Resolve the content def via the snapshot.
    let Ok(visual) = node_q.get(entity) else { return Harvestability::NotApplicable };
    let Some(node_def_id) = status
        .harvest_nodes
        .iter()
        .find(|n| n.node_id == visual.node_id)
        .map(|n| n.node_def_id.clone())
    else {
        return Harvestability::NotApplicable;
    };
    let Some(def) = harvest_db.get(&node_def_id) else { return Harvestability::NotApplicable };

    // Skill-level gate.
    let player_level = status
        .skill_entries
        .iter()
        .find(|s| s.skill_id == def.skill_id)
        .map(|s| s.level)
        .unwrap_or(1);
    if player_level < def.required_level {
        return Harvestability::LevelTooLow {
            required: def.required_level,
            skill_display: def.skill_display_name.clone(),
        };
    }

    // Tool gate.
    if let Some(required_tool) = def.required_tool.as_ref() {
        let tool_ok = player_q
            .single()
            .ok()
            .and_then(|equipment| equipment.main_hand.clone())
            .and_then(|id| item_db.get(&id).and_then(|item| item.tool.clone()))
            .map(|tool| &tool.kind == required_tool && tool.harvest_level >= def.required_level)
            .unwrap_or(false);
        if !tool_ok {
            return Harvestability::NeedTool {
                tool_name: min_tool_name_for_level(item_db, required_tool, def.required_level),
            };
        }
    }

    Harvestability::Ok
}

/// Display name of the lowest-tier tool of `kind` that can harvest `required_level`
/// (e.g. "Copper Axe"). Mirrors the server's message so they read identically.
fn min_tool_name_for_level(item_db: &ItemDb, kind: &str, required_level: u32) -> String {
    item_db
        .0
        .items
        .values()
        .filter_map(|item| item.tool.as_ref().map(|tool| (item, tool)))
        .filter(|(_, tool)| tool.kind == kind && tool.harvest_level >= required_level)
        .min_by_key(|(_, tool)| tool.harvest_level)
        .map(|(item, _)| item.name.clone())
        .unwrap_or_else(|| format!("better {kind}"))
}
