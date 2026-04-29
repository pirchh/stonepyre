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

    InteractionAck {
        accepted: bool,
        action: InteractionAction,
        target: InteractionTarget,
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
    ///
    /// Clients should use this for presentation so local visuals move forward
    /// along the server-approved route instead of chasing the previous server
    /// tile and occasionally bouncing backward.
    pub next_tile: Option<TilePos>,

    /// Current server-approved goal, if the player is moving.
    pub goal: Option<TilePos>,

    /// True while the server has an active movement goal/path for this player.
    pub moving: bool,
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
