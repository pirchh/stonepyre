use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Content-only harvest definitions (NO Bevy/server runtime types in here).
///
/// These are stable gameplay/content definitions like "oak_tree" or
/// "willow_tree". Runtime placement/state belongs to the world/server layer.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HarvestDefs {
    /// Keyed by stable id like "oak_tree", "willow_tree", "copper_rock", etc.
    pub nodes: HashMap<String, HarvestNodeDef>,

    /// Keyed by stable id like "woodcutting_oak_tree".
    pub loot_tables: HashMap<String, LootTableDef>,
}

impl HarvestDefs {
    pub fn node(&self, id: &str) -> Option<&HarvestNodeDef> {
        self.nodes.get(id)
    }

    pub fn loot_table(&self, id: &str) -> Option<&LootTableDef> {
        self.loot_tables.get(id)
    }
}

/// Defines how a harvestable node behaves.
///
/// This intentionally stays content-only. The server converts this into its
/// runtime catalog and keeps live state like charges/depletion separately.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HarvestNodeDef {
    pub id: String,
    pub display_name: String,

    /// Stable skill id, for example "woodcutting", "mining", "fishing".
    pub skill_id: String,

    /// UI-friendly skill name. Keeping this in content lets future skills be
    /// data-authored before the UI has a full skill registry.
    pub skill_display_name: String,

    /// What interaction verb drives this node.
    /// Examples: "ChopDown", "Mine", "Fish", "Gather".
    pub verb: String,

    /// Animation clip id string.
    /// Examples: "woodcutting", "mining", "fishing".
    pub clip: String,

    pub required_level: u32,
    pub xp_on_success: u32,
    pub base_success_chance: f32,
    /// How many successful yields until depleted. Kept as i32 for compatibility
    /// with the existing engine-side HarvestNode component.
    pub charges: i32,

    /// Respawn timer in seconds. Kept as f32 for compatibility with Bevy timers.
    pub respawn_seconds: f32,

    /// Loot table id resolved through `HarvestDefs::loot_tables`.
    pub loot_table: String,

    /// Game asset paths relative to `game/assets`.
    pub available_sprite: String,
    pub depleted_sprite: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootTableDef {
    pub id: String,
    pub entries: Vec<LootEntryDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LootEntryDef {
    pub item_id: String,
    pub min: u32,
    pub max: u32,
    pub weight: u32,
}
