use std::collections::HashMap;

use stonepyre_world::TilePos;

use crate::game::protocol::HarvestNodeSnapshot;

// Phase 7k-a demo content tuning.
//
// Keep this content/server-side for now. The DB should not become the map
// editor: placement comes from content/runtime data, live depletion state stays
// in server memory, and only inventory remains persisted.
//
// XP and level requirements are intentionally content-authored on the harvest
// node definition so future nodes can vary cleanly:
//
// oak_tree    -> Woodcutting, level 1, 10 XP
// willow_tree -> Woodcutting, level 20, 25 XP
const NORMAL_TREE_DEF_ID: &str = "oak_tree";
const NORMAL_TREE_DISPLAY_NAME: &str = "Oak Tree";
const NORMAL_TREE_REQUIRED_LEVEL: u32 = 1;
const NORMAL_TREE_XP_ON_SUCCESS: u32 = 10;
const NORMAL_TREE_SUCCESS_CHANCE: f32 = 0.62;
const NORMAL_TREE_CHARGES: u32 = 4;
const NORMAL_TREE_RESPAWN_SECS: u32 = 20;
const NORMAL_TREE_LOOT_TABLE_ID: &str = "woodcutting_oak_tree";
const NORMAL_TREE_AVAILABLE_SPRITE: &str = "world/harvest/oak_tree.png";
const NORMAL_TREE_DEPLETED_SPRITE: &str = "world/harvest/oak_tree_stump.png";

const NORMAL_TREE_LOOT_ITEM_ID: &str = "log";
const NORMAL_TREE_LOOT_MIN: u32 = 1;
const NORMAL_TREE_LOOT_MAX: u32 = 1;
const NORMAL_TREE_LOOT_WEIGHT: u32 = 100;

const LOG_ITEM_DISPLAY_NAME: &str = "Log";
const LOG_ITEM_INVENTORY_ICON: &str = "items/log.png";
const LOG_ITEM_STACKABLE: bool = true;

const DEMO_TREE_A_NODE_ID: &str = "demo_tree_2_0";
const DEMO_TREE_B_NODE_ID: &str = "demo_tree_4_1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestSkill {
    Woodcutting,
}

impl HarvestSkill {
    pub fn id(self) -> &'static str {
        match self {
            HarvestSkill::Woodcutting => "woodcutting",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            HarvestSkill::Woodcutting => "Woodcutting",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HarvestNodeDef {
    pub id: &'static str,
    pub display_name: &'static str,
    pub skill: HarvestSkill,
    pub required_level: u32,
    pub xp_on_success: u32,
    pub base_success_chance: f32,
    pub charges: u32,
    pub respawn_secs: u32,
    pub loot_table: &'static str,
    pub available_sprite: &'static str,
    pub depleted_sprite: &'static str,
}

#[derive(Clone, Debug)]
pub struct LootTable {
    pub id: &'static str,
    pub entries: Vec<LootEntry>,
}

#[derive(Clone, Debug)]
pub struct LootEntry {
    pub item_id: &'static str,
    pub min: u32,
    pub max: u32,
    pub weight: u32,
}

#[derive(Clone, Debug)]
pub struct ItemDef {
    pub id: &'static str,
    pub display_name: &'static str,
    pub inventory_icon: &'static str,
    pub stackable: bool,
}

#[derive(Clone, Debug)]
pub struct HarvestNodeInstance {
    pub node_id: &'static str,
    pub def_id: &'static str,
    pub tile: TilePos,
    pub charges_remaining: u32,
    pub depleted_until_tick: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct HarvestLootPreview {
    pub item_id: &'static str,
    pub quantity: u32,
}

#[derive(Clone, Debug)]
pub struct HarvestRollOutcome {
    pub node_id: &'static str,
    pub def_id: &'static str,
    pub display_name: &'static str,
    pub skill: HarvestSkill,
    pub xp_gained: u32,
    pub success: bool,
    pub depleted: bool,
    pub charges_remaining: u32,
    pub max_charges: u32,
    pub depleted_until_tick: Option<u64>,
    pub loot_preview: Option<HarvestLootPreview>,
}

pub struct HarvestCatalog {
    node_defs: HashMap<&'static str, HarvestNodeDef>,
    loot_tables: HashMap<&'static str, LootTable>,
    item_defs: HashMap<&'static str, ItemDef>,
    node_instances_by_tile: HashMap<TilePos, HarvestNodeInstance>,
}

impl HarvestCatalog {
    pub fn demo() -> Self {
        let mut node_defs = HashMap::new();
        node_defs.insert(
            NORMAL_TREE_DEF_ID,
            HarvestNodeDef {
                id: NORMAL_TREE_DEF_ID,
                display_name: NORMAL_TREE_DISPLAY_NAME,
                skill: HarvestSkill::Woodcutting,
                required_level: NORMAL_TREE_REQUIRED_LEVEL,
                xp_on_success: NORMAL_TREE_XP_ON_SUCCESS,
                base_success_chance: NORMAL_TREE_SUCCESS_CHANCE,
                charges: NORMAL_TREE_CHARGES,
                respawn_secs: NORMAL_TREE_RESPAWN_SECS,
                loot_table: NORMAL_TREE_LOOT_TABLE_ID,
                available_sprite: NORMAL_TREE_AVAILABLE_SPRITE,
                depleted_sprite: NORMAL_TREE_DEPLETED_SPRITE,
            },
        );

        let mut loot_tables = HashMap::new();
        loot_tables.insert(
            NORMAL_TREE_LOOT_TABLE_ID,
            LootTable {
                id: NORMAL_TREE_LOOT_TABLE_ID,
                entries: vec![LootEntry {
                    item_id: NORMAL_TREE_LOOT_ITEM_ID,
                    min: NORMAL_TREE_LOOT_MIN,
                    max: NORMAL_TREE_LOOT_MAX,
                    weight: NORMAL_TREE_LOOT_WEIGHT,
                }],
            },
        );

        let mut item_defs = HashMap::new();
        item_defs.insert(
            NORMAL_TREE_LOOT_ITEM_ID,
            ItemDef {
                id: NORMAL_TREE_LOOT_ITEM_ID,
                display_name: LOG_ITEM_DISPLAY_NAME,
                inventory_icon: LOG_ITEM_INVENTORY_ICON,
                stackable: LOG_ITEM_STACKABLE,
            },
        );

        let mut node_instances_by_tile = HashMap::new();

        for (node_id, tile) in [
            (DEMO_TREE_A_NODE_ID, TilePos::new(2, 0)),
            (DEMO_TREE_B_NODE_ID, TilePos::new(4, 1)),
        ] {
            node_instances_by_tile.insert(
                tile,
                HarvestNodeInstance {
                    node_id,
                    def_id: NORMAL_TREE_DEF_ID,
                    tile,
                    charges_remaining: NORMAL_TREE_CHARGES,
                    depleted_until_tick: None,
                },
            );
        }

        Self {
            node_defs,
            loot_tables,
            item_defs,
            node_instances_by_tile,
        }
    }

    pub fn node_at(&self, tile: TilePos) -> Option<&HarvestNodeInstance> {
        self.node_instances_by_tile.get(&tile)
    }

    pub fn node_def(&self, def_id: &str) -> Option<&HarvestNodeDef> {
        self.node_defs.get(def_id)
    }

    pub fn item_def(&self, item_id: &str) -> Option<&ItemDef> {
        self.item_defs.get(item_id)
    }

    pub fn loot_table(&self, table_id: &str) -> Option<&LootTable> {
        self.loot_tables.get(table_id)
    }

    pub fn node_def_at(&self, tile: TilePos) -> Option<&HarvestNodeDef> {
        let node = self.node_at(tile)?;
        self.node_def(node.def_id)
    }

    pub fn can_harvest_at(&self, tile: TilePos) -> bool {
        self.node_at(tile)
            .map(|node| node.charges_remaining > 0 && node.depleted_until_tick.is_none())
            .unwrap_or(false)
    }

    pub fn blocking_tiles(&self) -> impl Iterator<Item = TilePos> + '_ {
        self.node_instances_by_tile.keys().copied()
    }

    pub fn snapshots(&self) -> Vec<HarvestNodeSnapshot> {
        self.node_instances_by_tile
            .values()
            .filter_map(|node| self.snapshot_for_node(node))
            .collect()
    }

    pub fn snapshot_at(&self, tile: TilePos) -> Option<HarvestNodeSnapshot> {
        let node = self.node_instances_by_tile.get(&tile)?;
        self.snapshot_for_node(node)
    }

    fn snapshot_for_node(&self, node: &HarvestNodeInstance) -> Option<HarvestNodeSnapshot> {
        let def = self.node_defs.get(node.def_id)?;
        Some(HarvestNodeSnapshot {
            node_id: node.node_id.to_string(),
            node_def_id: node.def_id.to_string(),
            display_name: def.display_name.to_string(),
            tile: node.tile,
            charges_remaining: node.charges_remaining,
            max_charges: def.charges,
            depleted: node.charges_remaining == 0 || node.depleted_until_tick.is_some(),
            depleted_until_tick: node.depleted_until_tick,
            available_sprite: def.available_sprite.to_string(),
            depleted_sprite: def.depleted_sprite.to_string(),
        })
    }

    pub fn tick_respawns(&mut self, current_tick: u64) -> Vec<HarvestNodeSnapshot> {
        let mut restored = Vec::new();

        for node in self.node_instances_by_tile.values_mut() {
            let Some(depleted_until_tick) = node.depleted_until_tick else {
                continue;
            };

            if current_tick < depleted_until_tick {
                continue;
            }

            let Some(def) = self.node_defs.get(node.def_id) else {
                continue;
            };

            node.charges_remaining = def.charges;
            node.depleted_until_tick = None;

            restored.push(HarvestNodeSnapshot {
                node_id: node.node_id.to_string(),
                node_def_id: node.def_id.to_string(),
                display_name: def.display_name.to_string(),
                tile: node.tile,
                charges_remaining: node.charges_remaining,
                max_charges: def.charges,
                depleted: false,
                depleted_until_tick: None,
                available_sprite: def.available_sprite.to_string(),
                depleted_sprite: def.depleted_sprite.to_string(),
            });
        }

        restored
    }

    pub fn roll_harvest(
        &mut self,
        tile: TilePos,
        roll: f32,
        current_tick: u64,
        ticks_per_second: u64,
    ) -> Result<HarvestRollOutcome, String> {
        let (node_id, def_id, charges_remaining, depleted_until_tick) = {
            let Some(node) = self.node_instances_by_tile.get(&tile) else {
                return Err("target is not a harvest node".to_string());
            };

            (
                node.node_id,
                node.def_id,
                node.charges_remaining,
                node.depleted_until_tick,
            )
        };

        if charges_remaining == 0 || depleted_until_tick.is_some() {
            return Err("harvest node is depleted".to_string());
        }

        let Some(def) = self.node_defs.get(def_id).cloned() else {
            return Err(format!("missing harvest node def {def_id}"));
        };

        let chance = def.base_success_chance.clamp(0.0, 1.0);
        let success = roll < chance;

        let loot_preview = if success {
            self.loot_tables
                .get(def.loot_table)
                .and_then(|table| table.entries.first())
                .map(|entry| HarvestLootPreview {
                    item_id: entry.item_id,
                    quantity: entry.min,
                })
        } else {
            None
        };

        let node = self
            .node_instances_by_tile
            .get_mut(&tile)
            .ok_or_else(|| "target is not a harvest node".to_string())?;

        if success {
            node.charges_remaining = node.charges_remaining.saturating_sub(1);
            if node.charges_remaining == 0 {
                let respawn_ticks = u64::from(def.respawn_secs) * ticks_per_second.max(1);
                node.depleted_until_tick = Some(current_tick + respawn_ticks);
            }
        }

        Ok(HarvestRollOutcome {
            node_id,
            def_id,
            display_name: def.display_name,
            skill: def.skill,
            xp_gained: if success { def.xp_on_success } else { 0 },
            success,
            depleted: node.charges_remaining == 0,
            charges_remaining: node.charges_remaining,
            max_charges: def.charges,
            depleted_until_tick: node.depleted_until_tick,
            loot_preview,
        })
    }
}
