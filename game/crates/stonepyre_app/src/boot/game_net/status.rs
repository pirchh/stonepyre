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
    pub server_action: Option<PlayerActionSnapshot>,
    pub inventory_slots_total: usize,
    pub inventory_items: Vec<InventoryItemSnapshot>,
    pub inventory_dirty: bool,
    pub bag_slots: Vec<BagSlotSnapshot>,
    pub bag_slots_dirty: bool,
    pub skill_entries: Vec<SkillSnapshotEntry>,
    pub skills_dirty: bool,
    pub xp_feedback_queue: Vec<SkillXpFeedbackEntry>,
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
            server_action: None,
            inventory_slots_total: 20,
            inventory_items: Vec::new(),
            inventory_dirty: false,
            bag_slots: Vec::new(),
            bag_slots_dirty: false,
            skill_entries: Vec::new(),
            skills_dirty: false,
            xp_feedback_queue: Vec::new(),
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
