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
///
/// Loaded from `game/assets/world/harvest_objects/{skill}/{node}/manifest.json`.
/// `available_model` / `depleted_model` are computed from
/// `models.available` / `models.depleted` + the folder path during loading.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HarvestNodeDef {
    pub id: String,
    pub display_name: String,

    /// Stable skill id, for example "woodcutting", "mining", "fishing".
    pub skill_id: String,

    /// UI-friendly skill name.
    pub skill_display_name: String,

    /// What interaction verb drives this node.
    /// Examples: "ChopDown", "Mine", "Fish", "Harvest".
    pub verb: String,

    /// Animation clip id string.
    pub clip: String,

    pub required_level: u32,
    pub xp_on_success: u32,
    pub base_success_chance: f32,

    /// How many successful yields until depleted.
    pub charges: i32,

    /// Respawn timer in seconds.
    pub respawn_seconds: f32,

    /// Loot table id resolved through `HarvestDefs::loot_tables`.
    pub loot_table: String,

    /// Tool tag required in the player's MainHand slot to interact.
    /// Example: "axe" for trees, "pickaxe" for rocks, "fishing_rod" for fish.
    /// None means no tool is required.
    #[serde(default)]
    pub required_tool: Option<String>,

    /// Whether this node blocks player movement.
    #[serde(default = "default_true")]
    pub blocks_movement: bool,

    /// GLB paths relative to `game/assets/` — computed at load time.
    pub available_model: String,
    pub depleted_model: String,
}

fn default_true() -> bool { true }

/// On-disk format inside each `harvest_objects/{skill}/{node}/manifest.json`.
/// Loaded and converted to `HarvestNodeDef` by the content file loader.
#[derive(Clone, Debug, Deserialize)]
pub struct HarvestNodeManifest {
    pub id: String,
    pub display_name: String,
    pub skill_id: String,
    pub skill_display_name: String,
    pub verb: String,
    pub clip: String,
    pub required_level: u32,
    pub xp_on_success: u32,
    pub base_success_chance: f32,
    pub charges: i32,
    pub respawn_seconds: f32,
    pub loot_table: String,
    #[serde(default)]
    pub required_tool: Option<String>,
    #[serde(default = "default_true")]
    pub blocks_movement: bool,
    pub models: HarvestNodeModels,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HarvestNodeModels {
    pub available: String,
    pub depleted: String,
}

impl HarvestNodeManifest {
    /// Convert to `HarvestNodeDef`, constructing full game-asset-relative paths
    /// from the skill name and node folder name.
    ///
    /// `skill`  — e.g. `"woodcutting"`
    /// `folder` — e.g. `"oak"`
    pub fn into_def(self, skill: &str, folder: &str) -> HarvestNodeDef {
        let prefix = format!("world/harvest_objects/{}/{}", skill, folder);
        HarvestNodeDef {
            id: self.id,
            display_name: self.display_name,
            skill_id: self.skill_id,
            skill_display_name: self.skill_display_name,
            verb: self.verb,
            clip: self.clip,
            required_level: self.required_level,
            xp_on_success: self.xp_on_success,
            base_success_chance: self.base_success_chance,
            charges: self.charges,
            respawn_seconds: self.respawn_seconds,
            loot_table: self.loot_table,
            required_tool: self.required_tool,
            blocks_movement: self.blocks_movement,
            available_model: format!("{}/{}", prefix, self.models.available),
            depleted_model:  format!("{}/{}", prefix, self.models.depleted),
        }
    }
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
