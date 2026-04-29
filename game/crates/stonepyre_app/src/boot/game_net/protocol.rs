pub(super) use stonepyre_protocol::{
    ClientMsg,
    InteractionAction,
    InteractionTarget,
    PlayerSnapshot,
    ServerMsg,
    WorldSnapshot,
};

use stonepyre_world::TilePos;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NetPlayerSnapshot {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub tile: TilePos,
    pub next_tile: Option<TilePos>,
    pub goal: Option<TilePos>,
    pub moving: bool,
}
