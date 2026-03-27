use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Content-only definitions (NO Bevy types in here).
/// Engine wraps this in a Bevy Resource.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HarvestDefs {
    /// Keyed by stable id like "oak_tree", "willow_tree", "copper_rock", etc.
    pub nodes: HashMap<String, HarvestNodeDef>,
}

/// Defines how a harvestable node behaves.
/// This is intentionally generic so 20+ skills can plug in.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HarvestNodeDef {
    pub id: String,

    /// What verb drives this (engine maps verbs to clips, UI labels, etc).
    /// Examples: "ChopDown", "Mine", "Fish", "Gather".
    pub verb: String,

    /// Animation clip id string (engine maps clip ids to sprite folders).
    /// Examples: "woodcutting", "mining", "fishing".
    pub clip: String,

    /// How many successful “yields” until depleted.
    pub charges: i32,

    /// Respawn time in seconds once depleted.
    pub respawn_seconds: f32,

    /// Base chance (0..1) at level 1. You’ll later scale this by skill.
    pub base_success_chance: f32,

    /// XP per successful yield.
    pub xp: u32,

    /// What item is yielded on success (first pass: single drop id).
    pub drop_item_id: String,
}