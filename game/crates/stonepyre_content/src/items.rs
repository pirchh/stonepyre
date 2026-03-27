use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type ItemId = String;
pub type ContainerId = String;

/// Where an item is being stored (matters for stack rules).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageKind {
    Inventory,
    Bank,
    Container, // generic container like backpack/chest/etc
}

/// Stack policy is *contextual* (RuneScape rule: logs stack in bank, not in inventory).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StackPolicy {
    pub stack_in_inventory: bool,
    pub stack_in_bank: bool,
    pub stack_in_containers: bool,

    /// If stackable in a given storage, this is the max stack size.
    pub max_stack: u32,
}

impl Default for StackPolicy {
    fn default() -> Self {
        Self {
            stack_in_inventory: false,
            stack_in_bank: true,
            stack_in_containers: false,
            max_stack: 99_999,
        }
    }
}

impl StackPolicy {
    pub fn can_stack_in(&self, sk: StorageKind) -> bool {
        match sk {
            StorageKind::Inventory => self.stack_in_inventory,
            StorageKind::Bank => self.stack_in_bank,
            StorageKind::Container => self.stack_in_containers,
        }
    }
}

/// Equipment slots (RuneScape-ish, with your custom list).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Helm,
    Shoulders,
    Neck,
    Chest,
    Wrist,
    Gloves,
    Waist,
    Pants,
    Boots,
    Ring1,
    Ring2,

    /// Back slot: cape/backpack/etc (mutually exclusive).
    Back,
}

/// Optional equipment behavior definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EquipmentDef {
    pub slot: EquipSlot,

    /// If present, equipping this item gives you a wearable container (backpack).
    /// Example: "wooden_backpack"
    pub container_id: Option<ContainerId>,

    /// Placeholder stats hook (keep it simple for now).
    pub stats_tag: Option<String>,
}

/// Defines a “bag upgrade” that can be inserted into a backpack socket.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BagUpgradeDef {
    /// Extra slots granted when inserted.
    pub extra_slots: u32,
}

/// Defines a container type, like an inventory, backpack, chest, etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerDef {
    pub id: ContainerId,

    /// Base slot capacity.
    pub base_slots: u32,

    /// Number of upgrade sockets (like 4 misc bag slots).
    pub upgrade_sockets: u32,
}

/// Core item definition (content-only).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: String,

    /// Stack behavior differs by storage (inventory vs bank).
    pub stack_policy: StackPolicy,

    /// If equippable, includes slot and optional container link (backpack).
    pub equipment: Option<EquipmentDef>,

    /// If this item is a bag upgrade (for backpack sockets).
    pub bag_upgrade: Option<BagUpgradeDef>,

    /// Generic tags for later (tool tags, skill reqs, etc.)
    pub tags: Vec<String>,
}

/// Item definition database (content-only).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ItemDefs {
    pub items: HashMap<ItemId, ItemDef>,
}

impl ItemDefs {
    pub fn get(&self, id: &str) -> Option<&ItemDef> {
        self.items.get(id)
    }
}

/// Container definition database (content-only).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContainerDefs {
    pub containers: HashMap<ContainerId, ContainerDef>,
}

impl ContainerDefs {
    pub fn get(&self, id: &str) -> Option<&ContainerDef> {
        self.containers.get(id)
    }
}