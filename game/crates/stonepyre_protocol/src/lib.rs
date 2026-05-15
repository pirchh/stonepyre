use serde::{Deserialize, Serialize};
use stonepyre_world::TilePos;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    Ping,
    JoinWorld { character_id: Uuid },
    MoveTo { tile: TilePos },
    Interact {
        action: InteractionAction,
        target: InteractionTarget,
    },
    DropItem {
        slot_idx: usize,
        item_id: String,
        quantity: u32,
    },
    PickupGroundItem {
        ground_item_id: Uuid,
    },
    /// Place a bag item from the main inventory into a bag slot.
    EquipBag {
        /// Slot index in the main inventory containing the bag item.
        inventory_slot_idx: usize,
        /// Bag slot index (0 or 1).
        bag_slot: u8,
    },
    /// Remove the equipped bag from a bag slot back into inventory.
    UnequipBag {
        bag_slot: u8,
    },
    /// Move an item from the main inventory into an equipped bag.
    BagPutItem {
        bag_slot: u8,
        inventory_slot_idx: usize,
    },
    /// Move an item from an equipped bag back into the main inventory.
    BagTakeItem {
        bag_slot: u8,
        bag_item_slot_idx: usize,
    },
    /// Swap two slots within the main inventory.
    SwapInvSlots {
        from_slot: usize,
        to_slot: usize,
    },
    /// Move an item from one equipped bag to another.
    BagMoveItem {
        from_bag_slot: u8,
        from_item_slot: usize,
        to_bag_slot: u8,
    },
    /// Drag an inventory item into a specific bag slot index.
    BagPutItemToSlot {
        bag_slot: u8,
        inventory_slot_idx: usize,
        bag_item_slot_idx: usize,
    },
    /// Drag a bag item to a specific main inventory slot.
    BagTakeItemToSlot {
        bag_slot: u8,
        bag_item_slot_idx: usize,
        inv_slot_idx: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMsg {
    Pong,

    Welcome {
        player_id: Uuid,
        character_id: Uuid,
        tick_hz: u32,
    },

    Snapshot(WorldSnapshot),

    /// Immediate response to a submitted interaction intent.
    ///
    /// Accepted means the server recognized and stored/started the request. It
    /// does not necessarily mean the action is actively resolving this tick.
    /// Clients should use `PlayerActionSnapshot.state` from snapshots as the
    /// authoritative action lifecycle for presentation.
    InteractionAck {
        accepted: bool,
        action: InteractionAction,
        target: InteractionTarget,
        message: String,
    },

    /// Direct lifecycle event for client logs/UI. Snapshots remain the durable
    /// source of truth for active action state.
    ActionState {
        player_id: Uuid,
        action: InteractionAction,
        target: InteractionTarget,
        state: ActionState,
        message: String,
    },

    /// Structured harvest roll result. This is separate from ActionState so
    /// action lifecycle stays focused on queued/moving/active/complete state.
    HarvestResult(HarvestResult),

    /// Structured world-node event for depletion/restoration.
    HarvestNodeEvent(HarvestNodeEvent),

    InventorySnapshot(InventorySnapshot),

    InventoryDelta(InventoryDelta),

    GroundItemsSnapshot(GroundItemsSnapshot),

    GroundItemEvent(GroundItemEvent),

    SkillSnapshot(SkillSnapshot),

    SkillDelta(SkillDelta),

    /// Full state of both bag slots on join/refresh.
    BagSlotsSnapshot(BagSlotsSnapshot),

    /// A bag slot changed (equipped, unequipped, or item moved in/out).
    BagSlotChanged(BagSlotChanged),

    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub server_tick: u64,
    pub players: Vec<PlayerSnapshot>,
    pub harvest_nodes: Vec<HarvestNodeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub player_id: Uuid,
    pub character_id: Uuid,

    /// Last fully-authoritative tile reached by the server simulation.
    pub tile: TilePos,

    /// Next tile the server is currently moving this player toward, if any.
    pub next_tile: Option<TilePos>,

    /// Current server-approved movement goal, if the player is moving.
    pub goal: Option<TilePos>,

    /// True while the server has an active movement goal/path for this player.
    pub moving: bool,

    /// Fractional progress (0.0–1.0) toward `next_tile` within the current tile step.
    /// 0.0 = just reached `tile`, 1.0 = arrived at `next_tile`.
    /// Only meaningful when `moving` is true and `next_tile` is `Some`.
    pub move_progress: f32,

    /// Current server-owned non-movement action state for this player.
    pub action: Option<PlayerActionSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerActionSnapshot {
    pub action: InteractionAction,
    pub target: InteractionTarget,
    pub state: ActionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestNodeSnapshot {
    pub node_id: String,
    pub node_def_id: String,
    pub display_name: String,
    pub tile: TilePos,
    pub charges_remaining: u32,
    pub max_charges: u32,
    pub depleted: bool,
    pub depleted_until_tick: Option<u64>,

    /// Content-authored world sprite path for the available/harvestable state.
    ///
    /// The current client may still use color placeholders, but this lets the
    /// server snapshot carry the tree/stump presentation data before art is
    /// fully wired in.
    pub available_sprite: String,

    /// Content-authored world sprite path for the depleted state.
    pub depleted_sprite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestResult {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub action: InteractionAction,
    pub target: InteractionTarget,
    pub node_id: String,
    pub display_name: String,
    pub success: bool,
    pub item_id: Option<String>,
    pub quantity: u32,
    pub inventory_quantity: Option<i64>,
    pub charges_remaining: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestNodeEvent {
    pub kind: HarvestNodeEventKind,
    pub node_id: String,
    pub node_def_id: String,
    pub display_name: String,
    pub tile: TilePos,
    pub charges_remaining: u32,
    pub max_charges: u32,
    pub depleted_until_tick: Option<u64>,

    /// Content-authored world sprite path for the available/harvestable state.
    pub available_sprite: String,

    /// Content-authored world sprite path for the depleted state.
    pub depleted_sprite: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarvestNodeEventKind {
    Depleted,
    Restored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub character_id: Uuid,
    pub slots_total: usize,
    pub items: Vec<InventoryItemSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemSnapshot {
    pub slot_idx: usize,
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryDelta {
    pub character_id: Uuid,
    pub slot_idx: Option<usize>,
    pub item_id: String,
    pub quantity_delta: i64,
    pub new_quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundItemsSnapshot {
    pub items: Vec<GroundItemSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundItemSnapshot {
    pub ground_item_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
    pub tile: TilePos,
    pub owner_character_id: Option<Uuid>,
    pub despawn_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundItemEvent {
    pub kind: GroundItemEventKind,
    pub item: Option<GroundItemSnapshot>,
    pub ground_item_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundItemEventKind {
    Spawned,
    PickedUp,
    Despawned,
}

/// Full snapshot of both bag slots sent on JoinWorld.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagSlotsSnapshot {
    pub character_id: Uuid,
    pub slots: Vec<BagSlotSnapshot>,
}

/// State of a single bag slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagSlotSnapshot {
    pub bag_slot: u8,
    pub container_id: Uuid,
    /// Item id of the equipped bag, if any.
    pub equipped_item_id: Option<String>,
    /// Contents of the bag (empty if no bag is equipped).
    pub items: Vec<BagItemSnapshot>,
    pub slots_total: usize,
    /// Display name of the container def, if a bag is equipped.
    pub bag_display_name: Option<String>,
    /// Item type filter tag (None = general bag).
    pub item_type_filter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagItemSnapshot {
    pub slot_idx: usize,
    pub item_id: String,
    pub quantity: i64,
}

/// Sent after any bag slot mutation (equip/unequip/put/take).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagSlotChanged {
    pub character_id: Uuid,
    pub slot: BagSlotSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    pub character_id: Uuid,
    pub skills: Vec<SkillSnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshotEntry {
    pub skill_id: String,
    pub display_name: String,
    pub xp: i64,
    pub level: u32,
    pub xp_for_next_level: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDelta {
    pub character_id: Uuid,
    pub skill_id: String,
    pub display_name: String,
    pub xp_delta: i64,
    pub new_xp: i64,
    pub new_level: u32,
    pub xp_for_next_level: Option<i64>,

    /// Optional presentation source for client-side feedback.
    ///
    /// XP remains server-authoritative. This only tells the client where to
    /// place short-lived feedback such as floating harvest XP text.
    pub source: Option<SkillXpSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SkillXpSource {
    HarvestNode {
        node_id: String,
        tile: TilePos,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InteractionAction {
    WalkHere,
    ChopDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionTarget {
    Tile(TilePos),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionState {
    Queued,
    MovingToRange,
    Active,
    Cancelled,
    Complete,
    Rejected,
}
