use serde::{Deserialize, Serialize};
use stonepyre_world::TilePos;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMsg {
    Ping,
    JoinWorld { character_id: Uuid },
    MoveTo { tile: TilePos },
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
    pub tile: TilePos,
}
