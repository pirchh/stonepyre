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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarvestNodeEventKind {
    Depleted,
    Restored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub character_id: Uuid,
    pub items: Vec<InventoryItemSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItemSnapshot {
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryDelta {
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity_delta: i64,
    pub new_quantity: i64,
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
