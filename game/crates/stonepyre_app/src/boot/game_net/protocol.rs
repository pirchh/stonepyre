pub(super) use stonepyre_protocol::{ClientMsg, PlayerSnapshot, ServerMsg, WorldSnapshot};

use stonepyre_world::TilePos;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NetPlayerSnapshot {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub tile: TilePos,
}
