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
    SkillXpSource,
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
    BankSnapshot(BankSnapshot),
    BankTabChanged(BankTabSnapshot),
    /// Server's authoritative path in response to a MoveTo. The reconciler
    /// applies this in place of any locally-predicted path.
    PathConfirmed {
        goal: TilePos,
        tiles: Vec<TilePos>,
    },
    Error(String),
    Disconnected,
}

#[derive(Debug)]
pub enum GameNetCommand {
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
}

#[derive(Debug, Clone)]
pub struct SkillXpFeedbackEntry {
    pub skill_id: String,
    pub display_name: String,
    pub xp_delta: i64,
    pub new_xp: i64,
    pub new_level: u32,
    pub source: Option<SkillXpSource>,
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
    pub server_tile: Option<TilePos>,
    pub server_next_tile: Option<TilePos>,
    pub server_goal: Option<TilePos>,
    pub server_moving: bool,
    /// Fractional progress (0.0–1.0) the server has made toward `server_next_tile`.
    pub server_move_progress: f32,
    pub server_action: Option<PlayerActionSnapshot>,
    pub inventory_slots_total: usize,
    pub inventory_items: Vec<InventoryItemSnapshot>,
    pub inventory_dirty: bool,
    pub bag_slots: Vec<BagSlotSnapshot>,
    pub bag_slots_dirty: bool,
    pub skill_entries: Vec<SkillSnapshotEntry>,
    pub skills_dirty: bool,
    pub xp_feedback_queue: Vec<SkillXpFeedbackEntry>,
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
    pub remote_player_count: usize,
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
            server_tile: None,
            server_next_tile: None,
            server_goal: None,
            server_moving: false,
            server_move_progress: 0.0,
            server_action: None,
            inventory_slots_total: 20,
            inventory_items: Vec::new(),
            inventory_dirty: false,
            bag_slots: Vec::new(),
            bag_slots_dirty: false,
            skill_entries: Vec::new(),
            skills_dirty: false,
            xp_feedback_queue: Vec::new(),
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
            remote_player_count: 0,
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
