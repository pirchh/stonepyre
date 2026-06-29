use serde::{Deserialize, Serialize};
use stonepyre_world::TilePos;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    Ping,
    JoinWorld { character_id: Uuid },
    MoveTo { tile: TilePos },
    /// Continuous WASD movement input, sequence-stamped.
    /// `dx` = world-X axis, `dy` = world-Z axis (Vec2 convention used by the client).
    /// Send a normalised non-zero vec while keys are held; send `{0,0}` on key release.
    /// The server applies its own speed cap, so the magnitude is advisory only.
    /// `seq` is a per-client monotonic counter; the server echoes the last applied
    /// `seq` in PlayerSnapshot so the client can reconcile its local prediction.
    MoveDir { dx: f32, dy: f32, seq: u32 },
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
    /// Equip a worn item (axe, armour, …) from the main inventory. The server
    /// derives the destination slot from the item's equipment def and swaps any
    /// item already in that slot back into the freed inventory slot.
    EquipItem {
        inventory_slot_idx: usize,
        /// The item the client intends to equip. The server resolves the actual
        /// inventory slot by id, so a stale slot index still works.
        item_id: String,
    },
    /// Unequip the item in the given worn slot back into the main inventory.
    UnequipItem {
        /// Slot id, e.g. "main_hand" (matches EquipmentSlotSnapshot.slot).
        slot: String,
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

    // ------------------------------------------------------------------
    // Bank
    // ------------------------------------------------------------------

    /// Deposit one item from inventory into the bank.
    /// The server routes it to the correct tab via tag filters.
    BankDeposit {
        inv_slot_idx: usize,
        item_id: String,
        /// How many to deposit. Clamped to available quantity server-side.
        quantity: i64,
    },

    /// Withdraw items from a bank tab slot into inventory.
    /// quantity is clamped to available bank quantity and free inventory slots.
    BankWithdraw {
        tab_idx: u8,
        slot_idx: usize,
        item_id: String,
        quantity: i64,
    },

    /// Deposit every item in inventory into the bank.
    BankDepositAll,

    /// Create a new bank tab with the given display name and tag filters.
    /// Fails if the character already has 11 tabs (slots 1–11) or the
    /// filters overlap with an existing tab's filters.
    BankCreateTab {
        display_name: String,
        tag_filters: Vec<String>,
    },

    /// Rename or change the tag filters for an existing tab.
    BankUpdateTab {
        tab_idx: u8,
        display_name: String,
        tag_filters: Vec<String>,
    },

    /// Delete a bank tab. Its items are moved to tab 1 (General).
    BankDeleteTab {
        tab_idx: u8,
    },

    /// Move an item from one bank tab to another (manual re-organisation).
    BankMoveItem {
        from_tab_idx: u8,
        slot_idx: usize,
        item_id: String,
        to_tab_idx: u8,
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

    /// Full worn-equipment state sent on join and after any equip/unequip.
    EquipmentSnapshot(EquipmentSnapshot),

    /// Full bank state sent on UseBank interaction and after any bank mutation.
    BankSnapshot(BankSnapshot),

    /// A single bank tab changed (items deposited, withdrawn, tab renamed, etc.).
    BankTabChanged(BankTabSnapshot),

    /// Immediate response to a MoveTo request containing the server's authoritative
    /// path. The client should replace its locally-predicted path with this so both
    /// sides always follow the same tile sequence.
    PathConfirmed {
        /// The server's resolved goal tile (may differ from requested if blocked).
        goal: TilePos,
        /// Ordered list of tiles the server will walk through, not including the
        /// player's current tile. Empty when the player is already at the goal.
        tiles: Vec<TilePos>,
    },

    Error {
        message: String,
    },

    /// Player-facing notice produced in the tick loop (which only has the
    /// broadcast hub, not a per-connection sender). It rides the broadcast bus
    /// but is meant for one player: clients surface it only when `player_id`
    /// matches the local player, so it never leaks to everyone like `Error`.
    Notice {
        player_id: Uuid,
        message: String,
    },

    /// Authoritative set of unwalkable tiles, sent once on join. The client
    /// applies these to its WorldGrid so prediction and replay collide against
    /// exactly the tiles the server simulates.
    WorldCollision {
        blocked: Vec<TilePos>,
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

    /// Server-authoritative continuous world position (X axis).
    /// Replaces tile-interpolation for rendering; tile is still used for interaction distance.
    pub pos_x: f32,

    /// Server-authoritative continuous world position (Z axis).
    pub pos_z: f32,

    /// The `seq` of the last `MoveDir` the server had applied for this player when
    /// this snapshot was taken — the client reconciles its prediction against it.
    pub last_input_seq: u32,

    /// Last fully-authoritative tile — derived from pos each tick.
    /// Still used by harvest/bank/pickup distance checks.
    pub tile: TilePos,

    /// Next tile the server is currently moving this player toward, if any.
    pub next_tile: Option<TilePos>,

    /// Current server-approved movement goal, if the player is moving.
    pub goal: Option<TilePos>,

    /// True while the server has an active movement goal/path for this player.
    pub moving: bool,

    /// Fractional progress (0.0–1.0) toward `next_tile` within the current tile step.
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
    pub available_model: String,

    /// Content-authored world sprite path for the depleted state.
    pub depleted_model: String,
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
    pub available_model: String,

    /// Content-authored world sprite path for the depleted state.
    pub depleted_model: String,
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

/// State of one worn equipment slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentSlotSnapshot {
    /// Slot id, e.g. "main_hand", "helm". Matches EquipSlot serialized lowercase.
    pub slot: String,
    pub item_id: String,
}

/// Full snapshot of a character's worn equipment. Sent on JoinWorld and after
/// every equip/unequip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipmentSnapshot {
    pub character_id: Uuid,
    pub slots: Vec<EquipmentSlotSnapshot>,
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
    Harvest,
    UseBank,
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

// ------------------------------------------------------------------
// Bank
// ------------------------------------------------------------------

/// Full bank state for a character. Tab 0 is the implicit "All" view;
/// the tabs vec contains the physical storage tabs (indices 1–11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankSnapshot {
    pub character_id: Uuid,
    pub tabs: Vec<BankTabSnapshot>,
}

/// State of one physical bank tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankTabSnapshot {
    pub character_id: Uuid,
    pub tab_idx: u8,
    pub display_name: String,
    /// Item tag filters. Empty = accept anything (General tab).
    pub tag_filters: Vec<String>,
    pub items: Vec<BankItemSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankItemSnapshot {
    pub slot_idx: usize,
    pub item_id: String,
    pub quantity: i64,
}
