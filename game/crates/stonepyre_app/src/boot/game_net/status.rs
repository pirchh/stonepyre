use bevy::prelude::*;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use uuid::Uuid;

use stonepyre_world::TilePos;

use super::protocol::{
    ActionState,
    BagSlotChanged,
    BagSlotSnapshot,
    BagSlotsSnapshot,
    BankItemSnapshot,
    BankSnapshot,
    BankTabSnapshot,
    EquipmentSlotSnapshot,
    EquipmentSnapshot,
    GroundItemEvent,
    GroundItemSnapshot,
    GroundItemsSnapshot,
    HarvestNodeEvent,
    HarvestNodeSnapshot,
    HarvestResult,
    InteractionAction,
    InteractionTarget,
    InventoryDelta,
    InventoryItemSnapshot,
    InventorySnapshot,
    NetPlayerSnapshot,
    PlayerActionSnapshot,
    SkillDelta,
    SkillSnapshot,
    SkillSnapshotEntry,
};

#[derive(Debug)]
pub enum GameNetEvent {
    Connecting { url: String, character_id: Uuid },
    Connected,
    Welcome {
        player_id: Uuid,
        character_id: Uuid,
        tick_hz: u32,
    },
    Snapshot {
        server_tick: u64,
        players: Vec<NetPlayerSnapshot>,
        harvest_nodes: Vec<HarvestNodeSnapshot>,
        /// Continuous server position for the local player, if present.
        server_pos: Option<[f32; 2]>,
        server_tile: Option<TilePos>,
        server_next_tile: Option<TilePos>,
        server_goal: Option<TilePos>,
        server_moving: bool,
        server_move_progress: f32,
        server_action: Option<PlayerActionSnapshot>,
    },
    MoveSent { tile: TilePos },
    InteractionAck {
        accepted: bool,
        action: InteractionAction,
        target: InteractionTarget,
        message: String,
    },
    ActionState {
        player_id: Uuid,
        action: InteractionAction,
        target: InteractionTarget,
        state: ActionState,
        message: String,
    },
    HarvestResult(HarvestResult),
    HarvestNodeEvent(HarvestNodeEvent),
    InventorySnapshot(InventorySnapshot),
    InventoryDelta(InventoryDelta),
    GroundItemsSnapshot(GroundItemsSnapshot),
    GroundItemEvent(GroundItemEvent),
    SkillSnapshot(SkillSnapshot),
    SkillDelta(SkillDelta),
    BagSlotsSnapshot(BagSlotsSnapshot),
    BagSlotChanged(BagSlotChanged),
    EquipmentSnapshot(EquipmentSnapshot),
    BankSnapshot(BankSnapshot),
    BankTabChanged(BankTabSnapshot),
    /// Server's authoritative path in response to a MoveTo. The reconciler
    /// applies this in place of any locally-predicted path.
    PathConfirmed {
        goal: TilePos,
        tiles: Vec<TilePos>,
    },
    Error(String),
    /// A player-facing notice from the server (action rejected, gate failed,
    /// inventory full, etc.) — surfaced as a right-side feedback drop. Distinct
    /// from `Error`, which is for transport/parse failures and is logged only.
    /// `player_id`: `None` = arrived on our own connection (always ours);
    /// `Some` = broadcast for a specific player, shown only if it's us.
    ServerNotice { player_id: Option<Uuid>, message: String },
    /// Authoritative blocked-tile set from the server, applied to WorldGrid so
    /// client prediction/replay collide against exactly what the server sims.
    WorldCollision(Vec<TilePos>),
    Disconnected,
}

#[derive(Debug)]
pub enum GameNetCommand {
    /// Continuous WASD direction — sent on key change. `seq` is the per-client
    /// monotonic input counter (for server-reconciliation).
    MoveDir { dx: f32, dy: f32, seq: u32 },
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
    EquipBag {
        inventory_slot_idx: usize,
        bag_slot: u8,
    },
    UnequipBag {
        bag_slot: u8,
    },
    EquipItem {
        inventory_slot_idx: usize,
        item_id: String,
    },
    UnequipItem {
        slot: String,
    },
    BagPutItem {
        bag_slot: u8,
        inventory_slot_idx: usize,
    },
    BagTakeItem {
        bag_slot: u8,
        bag_item_slot_idx: usize,
    },
    SwapInvSlots {
        from_slot: usize,
        to_slot: usize,
    },
    BagMoveItem {
        from_bag_slot: u8,
        from_item_slot: usize,
        to_bag_slot: u8,
    },
    BagPutItemToSlot {
        bag_slot: u8,
        inventory_slot_idx: usize,
        bag_item_slot_idx: usize,
    },
    BagTakeItemToSlot {
        bag_slot: u8,
        bag_item_slot_idx: usize,
        inv_slot_idx: usize,
    },
    /// Deposit a single inventory slot into the bank.
    BankDeposit {
        inv_slot_idx: usize,
        item_id: String,
        quantity: u32,
    },
    /// Withdraw items from a specific bank tab slot into inventory.
    BankWithdraw {
        tab_idx: u8,
        slot_idx: usize,
        item_id: String,
        quantity: u32,
    },
    /// Deposit every item in inventory into the bank.
    BankDepositAll,
    /// Close the bank panel client-side (no server message needed).
    BankClose,
    /// Create a new bank tab with the given name.
    BankCreateTab { display_name: String },
}

/// One entry in the right-side feedback drop stack (XP gains, item gains, and
/// short status messages such as harvest-gate rejections).
#[derive(Debug, Clone)]
pub enum FeedbackDrop {
    /// "+{amount} {skill_display} XP"
    Xp { skill_display: String, amount: i64 },
    /// "[icon] +{quantity} {item name}" — icon/name resolved from ItemDb.
    Item { item_id: String, quantity: u32 },
    /// Plain status text, e.g. "Need a Copper Axe".
    Message { text: String },
}

#[derive(Resource, Debug, Clone)]
pub struct GameNetStatus {
    pub connected: bool,
    pub connecting: bool,
    pub player_id: Option<Uuid>,
    pub character_id: Option<Uuid>,
    pub tick_hz: Option<u32>,
    pub server_tick: Option<u64>,
    pub snapshot_players: usize,
    pub latest_players: Vec<NetPlayerSnapshot>,
    pub harvest_nodes: Vec<HarvestNodeSnapshot>,
    pub ground_items: Vec<GroundItemSnapshot>,
    pub ground_items_dirty: bool,
    /// Server-authoritative continuous world position for the local player.
    pub server_pos: Option<[f32; 2]>,
    pub server_tile: Option<TilePos>,
    pub server_next_tile: Option<TilePos>,
    pub server_goal: Option<TilePos>,
    pub server_moving: bool,
    /// Fractional progress (0.0–1.0) the server has made toward `server_next_tile`.
    pub server_move_progress: f32,
    pub server_action: Option<PlayerActionSnapshot>,
    /// True while a Harvest action is live on the server: set on the first
    /// Queued/MovingToRange/Active event and cleared on the terminal state
    /// (Complete/Cancelled/Rejected). Because the loop is server-driven, the
    /// server keeps the action Active across every swing and only sends a
    /// terminal state when the whole session ends — so the falling edge of this
    /// flag is the single "stop animating" signal on the client.
    ///
    /// Driven by ActionState events only — NOT by 10Hz Snapshots — so it is
    /// immune to the race where a Snapshot arrives with server_action=None
    /// before the terminal ActionState is delivered.
    pub action_event_in_flight: bool,
    pub inventory_slots_total: usize,
    pub inventory_items: Vec<InventoryItemSnapshot>,
    pub inventory_dirty: bool,
    pub bag_slots: Vec<BagSlotSnapshot>,
    pub bag_slots_dirty: bool,
    /// Worn equipment slots (main-hand, …). Applied to the player's Equipment
    /// component by sync_equipment_from_server when dirty.
    pub equipment: Vec<EquipmentSlotSnapshot>,
    pub equipment_dirty: bool,
    pub skill_entries: Vec<SkillSnapshotEntry>,
    pub skills_dirty: bool,
    pub feedback_drops: Vec<FeedbackDrop>,
    /// Bank tabs (physical tabs 1–11). Empty until first UseBank interaction.
    pub bank_tabs: Vec<BankTabSnapshot>,
    pub bank_dirty: bool,
    /// Whether the bank UI panel is currently open.
    pub bank_open: bool,
    /// Pending server-authoritative path waiting to be applied by the reconciler.
    /// Set by the pump, consumed (and cleared) by reconcile_local_player_to_server.
    pub pending_server_path: Option<(TilePos, Vec<TilePos>)>,
    /// Number of PathConfirmed responses still in flight (incremented on every
    /// MoveTo sent, decremented on every PathConfirmed received). The reconciler
    /// suppresses follow-target heuristics while this is > 0 so it doesn't BFS
    /// toward server_next_tile (which may be on the wrong side of an obstacle)
    /// during the RTT window before the confirmed path arrives. A counter rather
    /// than a bool handles rapid clicks: N clicks → N in-flight responses, and
    /// heuristics stay suppressed until the last PathConfirmed arrives.
    pub pending_path_confirmations: u32,
    pub local_tile: Option<TilePos>,
    pub drift_tiles: Option<i32>,
    pub last_move_sent: Option<TilePos>,
    pub action_marker_target: Option<TilePos>,
    pub last_error: Option<String>,
    pub correction_count: u64,
    /// The latest input `seq` the client has sent (set by send_wasd_movement).
    /// The reconciler compares it to the server's echoed `last_input_seq` to tell
    /// whether the authoritative position already reflects our newest direction.
    pub last_sent_input_seq: u32,
    pub remote_player_count: usize,
    /// True after the first authoritative position has been applied.
    /// Suppresses the hard-snap warning on initial load.
    pub initial_sync_done: bool,
}

impl Default for GameNetStatus {
    fn default() -> Self {
        Self {
            connected: false,
            connecting: false,
            player_id: None,
            character_id: None,
            tick_hz: None,
            server_tick: None,
            snapshot_players: 0,
            latest_players: Vec::new(),
            harvest_nodes: Vec::new(),
            ground_items: Vec::new(),
            ground_items_dirty: false,
            server_pos: None,
            server_tile: None,
            server_next_tile: None,
            server_goal: None,
            server_moving: false,
            server_move_progress: 0.0,
            server_action: None,
            action_event_in_flight: false,
            inventory_slots_total: 20,
            inventory_items: Vec::new(),
            inventory_dirty: false,
            bag_slots: Vec::new(),
            bag_slots_dirty: false,
            equipment: Vec::new(),
            equipment_dirty: false,
            skill_entries: Vec::new(),
            skills_dirty: false,
            feedback_drops: Vec::new(),
            bank_tabs: Vec::new(),
            bank_dirty: false,
            bank_open: false,
            pending_server_path: None,
            pending_path_confirmations: 0,
            local_tile: None,
            drift_tiles: None,
            last_move_sent: None,
            action_marker_target: None,
            last_error: None,
            correction_count: 0,
            last_sent_input_seq: 0,
            remote_player_count: 0,
            initial_sync_done: false,
        }
    }
}

/// Runtime bridge for the server-side game websocket.
#[derive(Resource)]
pub struct GameNetRuntime {
    pub tx: Sender<GameNetEvent>,
    pub rx: Mutex<Receiver<GameNetEvent>>,
    pub(super) command_tx: Mutex<Option<Sender<GameNetCommand>>>,
}

impl Default for GameNetRuntime {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
            command_tx: Mutex::new(None),
        }
    }
}
