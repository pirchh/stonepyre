use bevy::prelude::*;
use stonepyre_content::items::{
    ContainerDef, ContainerDefs, ContainerId, EquipSlot, ItemDef, ItemDefs, ItemId, StorageKind,
};

/// Runtime stack. Even if most items aren't stackable in inventory,
/// bank stacking will use qty > 1.
#[derive(Clone, Debug)]
pub struct ItemStack {
    pub id: ItemId,
    pub qty: u32,
}

/// A generic runtime container with fixed slots (Option = empty).
#[derive(Component, Clone, Debug)]
pub struct Container {
    pub kind: StorageKind,
    pub slots: Vec<Option<ItemStack>>,
}

impl Container {
    pub fn new(kind: StorageKind, slot_count: u32) -> Self {
        Self {
            kind,
            slots: vec![None; slot_count as usize],
        }
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }

    pub fn has_any_items(&self) -> bool {
        self.slots.iter().any(|s| s.is_some())
    }
}

/// Player's main inventory container (16 slots).
#[derive(Component, Clone, Debug)]
pub struct Inventory {
    pub container: Container,
}

impl Inventory {
    pub fn new(slot_count: u32) -> Self {
        Self {
            container: Container::new(StorageKind::Inventory, slot_count),
        }
    }

    pub fn is_full(&self) -> bool {
        self.container.is_full()
    }
}

/// Equipped gear slots. (Separate from inventory, RuneScape style.)
#[derive(Component, Clone, Debug, Default)]
pub struct Equipment {
    pub helm: Option<ItemId>,
    pub shoulders: Option<ItemId>,
    pub neck: Option<ItemId>,
    pub chest: Option<ItemId>,
    pub wrist: Option<ItemId>,
    pub gloves: Option<ItemId>,
    pub waist: Option<ItemId>,
    pub pants: Option<ItemId>,
    pub boots: Option<ItemId>,
    pub ring1: Option<ItemId>,
    pub ring2: Option<ItemId>,
    pub back: Option<ItemId>,
}

impl Equipment {
    pub fn set_slot(&mut self, slot: EquipSlot, item: Option<ItemId>) {
        match slot {
            EquipSlot::Helm => self.helm = item,
            EquipSlot::Shoulders => self.shoulders = item,
            EquipSlot::Neck => self.neck = item,
            EquipSlot::Chest => self.chest = item,
            EquipSlot::Wrist => self.wrist = item,
            EquipSlot::Gloves => self.gloves = item,
            EquipSlot::Waist => self.waist = item,
            EquipSlot::Pants => self.pants = item,
            EquipSlot::Boots => self.boots = item,
            EquipSlot::Ring1 => self.ring1 = item,
            EquipSlot::Ring2 => self.ring2 = item,
            EquipSlot::Back => self.back = item,
        }
    }

    pub fn get_slot(&self, slot: EquipSlot) -> Option<&ItemId> {
        match slot {
            EquipSlot::Helm => self.helm.as_ref(),
            EquipSlot::Shoulders => self.shoulders.as_ref(),
            EquipSlot::Neck => self.neck.as_ref(),
            EquipSlot::Chest => self.chest.as_ref(),
            EquipSlot::Wrist => self.wrist.as_ref(),
            EquipSlot::Gloves => self.gloves.as_ref(),
            EquipSlot::Waist => self.waist.as_ref(),
            EquipSlot::Pants => self.pants.as_ref(),
            EquipSlot::Boots => self.boots.as_ref(),
            EquipSlot::Ring1 => self.ring1.as_ref(),
            EquipSlot::Ring2 => self.ring2.as_ref(),
            EquipSlot::Back => self.back.as_ref(),
        }
    }
}

/// If the player has a backpack equipped, this points to the backpack container entity.
#[derive(Component, Clone, Debug, Default)]
pub struct EquippedBackpack {
    pub entity: Option<Entity>,
    pub container_id: Option<ContainerId>,
}

// ------------------------------------------------------------
// Toolbelt (New World-ish tool slots)
// ------------------------------------------------------------

/// 2 rows × 7 slots.
pub const TOOLBELT_SLOT_COUNT: usize = 14;

/// Tool kinds that skills can reference. Multiple skills can share tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Axe,
    Pickaxe,
    Rod,
    Knife,
    Hammer,
    Chisel,
    Needle,
    Saw,
    Tongs,
    Mortar,
    Brush,
    Sickle,
    Pan,
    Lantern,
}

impl ToolKind {
    /// Stable ordering (also UI ordering).
    pub const ORDER: [ToolKind; TOOLBELT_SLOT_COUNT] = [
        ToolKind::Axe,
        ToolKind::Pickaxe,
        ToolKind::Rod,
        ToolKind::Knife,
        ToolKind::Hammer,
        ToolKind::Chisel,
        ToolKind::Needle,
        ToolKind::Saw,
        ToolKind::Tongs,
        ToolKind::Mortar,
        ToolKind::Brush,
        ToolKind::Sickle,
        ToolKind::Pan,
        ToolKind::Lantern,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            ToolKind::Axe => "Axe",
            ToolKind::Pickaxe => "Pickaxe",
            ToolKind::Rod => "Rod",
            ToolKind::Knife => "Knife",
            ToolKind::Hammer => "Hammer",
            ToolKind::Chisel => "Chisel",
            ToolKind::Needle => "Needle",
            ToolKind::Saw => "Saw",
            ToolKind::Tongs => "Tongs",
            ToolKind::Mortar => "Mortar",
            ToolKind::Brush => "Brush",
            ToolKind::Sickle => "Sickle",
            ToolKind::Pan => "Pan",
            ToolKind::Lantern => "Lantern",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "Axe" => ToolKind::Axe,
            "Pickaxe" => ToolKind::Pickaxe,
            "Rod" => ToolKind::Rod,
            "Knife" => ToolKind::Knife,
            "Hammer" => ToolKind::Hammer,
            "Chisel" => ToolKind::Chisel,
            "Needle" => ToolKind::Needle,
            "Saw" => ToolKind::Saw,
            "Tongs" => ToolKind::Tongs,
            "Mortar" => ToolKind::Mortar,
            "Brush" => ToolKind::Brush,
            "Sickle" => ToolKind::Sickle,
            "Pan" => ToolKind::Pan,
            "Lantern" => ToolKind::Lantern,
            _ => return None,
        })
    }

    #[inline]
    pub fn index(self) -> usize {
        for (i, k) in Self::ORDER.iter().copied().enumerate() {
            if k == self {
                return i;
            }
        }
        0
    }
}

/// Equipped tools (separate from gear).
#[derive(Component, Clone, Debug)]
pub struct Toolbelt {
    pub slots: [Option<ItemId>; TOOLBELT_SLOT_COUNT],
}

impl Default for Toolbelt {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}

impl Toolbelt {
    pub fn set(&mut self, kind: ToolKind, item: Option<ItemId>) {
        self.slots[kind.index()] = item;
    }

    pub fn get(&self, kind: ToolKind) -> Option<&ItemId> {
        self.slots[kind.index()].as_ref()
    }

    pub fn get_by_id(&self, id: &str) -> Option<&ItemId> {
        ToolKind::from_id(id).and_then(|k| self.get(k))
    }
}

// ------------------------------------------------------------
// Content resources (engine wraps content defs in Resources).
// ------------------------------------------------------------

#[derive(Resource, Clone, Debug)]
pub struct ItemDb(pub ItemDefs);

#[derive(Resource, Clone, Debug)]
pub struct ContainerDb(pub ContainerDefs);

impl ItemDb {
    pub fn get(&self, id: &str) -> Option<&ItemDef> {
        self.0.get(id)
    }
}

impl ContainerDb {
    pub fn get(&self, id: &str) -> Option<&ContainerDef> {
        self.0.get(id)
    }
}

/// Rule you specified:
/// If backpack has items in its internal slots, you cannot unequip it.
pub fn can_unequip_backpack(backpack_container: &Container) -> bool {
    !backpack_container.has_any_items()
}