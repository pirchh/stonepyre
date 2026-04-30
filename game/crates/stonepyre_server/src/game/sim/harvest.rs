use std::collections::HashMap;

use stonepyre_world::TilePos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarvestSkill {
    Woodcutting,
}

#[derive(Clone, Debug)]
pub struct HarvestNodeDef {
    pub id: &'static str,
    pub display_name: &'static str,
    pub skill: HarvestSkill,
    pub required_level: u32,
    pub base_success_chance: f32,
    pub charges: u32,
    pub respawn_secs: u32,
    pub loot_table: &'static str,
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
    pub success: bool,
    pub depleted: bool,
    pub charges_remaining: u32,
    pub loot_preview: Option<HarvestLootPreview>,
}

pub struct HarvestCatalog {
    node_defs: HashMap<&'static str, HarvestNodeDef>,
    loot_tables: HashMap<&'static str, LootTable>,
    node_instances_by_tile: HashMap<TilePos, HarvestNodeInstance>,
}

impl HarvestCatalog {
    pub fn demo() -> Self {
        let mut node_defs = HashMap::new();
        node_defs.insert(
            "tree_normal",
            HarvestNodeDef {
                id: "tree_normal",
                display_name: "Tree",
                skill: HarvestSkill::Woodcutting,
                required_level: 1,
                base_success_chance: 0.55,
                charges: 3,
                respawn_secs: 30,
                loot_table: "woodcutting_tree_normal",
            },
        );

        let mut loot_tables = HashMap::new();
        loot_tables.insert(
            "woodcutting_tree_normal",
            LootTable {
                id: "woodcutting_tree_normal",
                entries: vec![LootEntry {
                    item_id: "log",
                    min: 1,
                    max: 1,
                    weight: 100,
                }],
            },
        );

        let mut node_instances_by_tile = HashMap::new();
        node_instances_by_tile.insert(
            TilePos::new(2, 0),
            HarvestNodeInstance {
                node_id: "demo_tree_2_0",
                def_id: "tree_normal",
                tile: TilePos::new(2, 0),
                charges_remaining: 3,
                depleted_until_tick: None,
            },
        );

        Self {
            node_defs,
            loot_tables,
            node_instances_by_tile,
        }
    }

    pub fn node_at(&self, tile: TilePos) -> Option<&HarvestNodeInstance> {
        self.node_instances_by_tile.get(&tile)
    }

    pub fn node_def(&self, def_id: &str) -> Option<&HarvestNodeDef> {
        self.node_defs.get(def_id)
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

    pub fn roll_harvest(
        &mut self,
        tile: TilePos,
        roll: f32,
        current_tick: u64,
    ) -> Result<HarvestRollOutcome, String> {
        let (node_id, def_id, charges_remaining) = {
            let Some(node) = self.node_instances_by_tile.get(&tile) else {
                return Err("target is not a harvest node".to_string());
            };

            (node.node_id, node.def_id, node.charges_remaining)
        };

        if charges_remaining == 0 {
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
                node.depleted_until_tick = Some(current_tick + u64::from(def.respawn_secs));
            }
        }

        Ok(HarvestRollOutcome {
            node_id,
            def_id,
            display_name: def.display_name,
            success,
            depleted: node.charges_remaining == 0,
            charges_remaining: node.charges_remaining,
            loot_preview,
        })
    }
}
