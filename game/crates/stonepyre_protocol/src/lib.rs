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

    /// Optional direct lifecycle event for client logs/UI. Snapshots remain the
    /// durable source of truth for active action state.
    ActionState {
        player_id: Uuid,
        action: InteractionAction,
        target: InteractionTarget,
        state: ActionState,
        message: String,
    },

    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub server_tick: u64,
    pub players: Vec<PlayerSnapshot>,
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
