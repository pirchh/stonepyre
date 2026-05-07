use crate::items::{
    BagUpgradeDef, ContainerDef, ContainerDefs, EquipmentDef, EquipSlot, ItemDef, ItemDefs,
    StackPolicy,
};
use crate::objects::{HarvestDefs, HarvestNodeDef, LootEntryDef, LootTableDef};

/// A single struct you can load into the engine/server as a content db.
#[derive(Clone, Debug)]
pub struct ContentDb {
    pub items: ItemDefs,
    pub containers: ContainerDefs,
    pub harvest: HarvestDefs,
}

pub fn default_item_defs() -> ItemDefs {
    let mut defs = ItemDefs::default();

    // Basic log yielded by current woodcutting harvest nodes.
    defs.items.insert(
        "log".to_string(),
        ItemDef {
            id: "log".to_string(),
            name: "Log".to_string(),
            stack_policy: StackPolicy {
                stack_in_inventory: true,
                stack_in_bank: true,
                stack_in_containers: true,
                max_stack: 99_999,
            },
            equipment: None,
            bag_upgrade: None,
            tags: vec!["material".to_string(), "wood".to_string()],
        },
    );

    // Legacy/content example: Oak log. Keep this available while harvest output
    // remains on the current runtime "log" item id.
    defs.items.insert(
        "log_oak".to_string(),
        ItemDef {
            id: "log_oak".to_string(),
            name: "Oak Log".to_string(),
            stack_policy: StackPolicy {
                stack_in_inventory: false,
                stack_in_bank: true,
                stack_in_containers: false,
                max_stack: 99_999,
            },
            equipment: None,
            bag_upgrade: None,
            tags: vec!["material".to_string(), "wood".to_string()],
        },
    );

    // Wooden Backpack (equip in back slot) → grants container "wooden_backpack".
    defs.items.insert(
        "backpack_wooden".to_string(),
        ItemDef {
            id: "backpack_wooden".to_string(),
            name: "Wooden Backpack".to_string(),
            stack_policy: StackPolicy {
                stack_in_inventory: false,
                stack_in_bank: false,
                stack_in_containers: false,
                max_stack: 1,
            },
            equipment: Some(EquipmentDef {
                slot: EquipSlot::Back,
                container_id: Some("wooden_backpack".to_string()),
                stats_tag: None,
            }),
            bag_upgrade: None,
            tags: vec!["container".to_string(), "backpack".to_string()],
        },
    );

    // Small Bag Upgrade (+2 slots), intended to be inserted into backpack sockets.
    defs.items.insert(
        "bag_small".to_string(),
        ItemDef {
            id: "bag_small".to_string(),
            name: "Small Bag".to_string(),
            stack_policy: StackPolicy {
                stack_in_inventory: false,
                stack_in_bank: false,
                stack_in_containers: false,
                max_stack: 1,
            },
            equipment: None,
            bag_upgrade: Some(BagUpgradeDef { extra_slots: 2 }),
            tags: vec!["bag_upgrade".to_string()],
        },
    );

    defs
}

pub fn default_container_defs() -> ContainerDefs {
    let mut defs = ContainerDefs::default();

    // Backpack: 6 base slots + 4 upgrade sockets.
    defs.containers.insert(
        "wooden_backpack".to_string(),
        ContainerDef {
            id: "wooden_backpack".to_string(),
            base_slots: 6,
            upgrade_sockets: 4,
        },
    );

    defs
}

pub fn default_harvest_defs() -> HarvestDefs {
    let mut defs = HarvestDefs::default();

    defs.nodes.insert(
        "oak_tree".to_string(),
        HarvestNodeDef {
            id: "oak_tree".to_string(),
            display_name: "Oak Tree".to_string(),
            skill_id: "woodcutting".to_string(),
            skill_display_name: "Woodcutting".to_string(),
            verb: "ChopDown".to_string(),
            clip: "woodcutting".to_string(),
            required_level: 1,
            xp_on_success: 10,
            base_success_chance: 0.62,
            charges: 4,
            respawn_seconds: 20.0,
            loot_table: "woodcutting_oak_tree".to_string(),
            available_sprite: "world/skills/woodcutting/harvest_nodes/oak_tree/available.png".to_string(),
            depleted_sprite: "world/skills/woodcutting/harvest_nodes/oak_tree/depleted.png".to_string(),
        },
    );

    defs.loot_tables.insert(
        "woodcutting_oak_tree".to_string(),
        LootTableDef {
            id: "woodcutting_oak_tree".to_string(),
            entries: vec![LootEntryDef {
                item_id: "log".to_string(),
                min: 1,
                max: 1,
                weight: 100,
            }],
        },
    );

    defs
}

pub fn default_content_db() -> ContentDb {
    ContentDb {
        items: default_item_defs(),
        containers: default_container_defs(),
        harvest: default_harvest_defs(),
    }
}
