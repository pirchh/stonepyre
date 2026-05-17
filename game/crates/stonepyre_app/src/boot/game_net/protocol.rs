pub(super) use stonepyre_protocol::{
    ActionState,
    BagItemSnapshot,
    BagSlotChanged,
    BagSlotSnapshot,
    BagSlotsSnapshot,
    BankItemSnapshot,
    BankSnapshot,
    BankTabSnapshot,
    ClientMsg,
    GroundItemEvent,
    GroundItemEventKind,
    GroundItemSnapshot,
    GroundItemsSnapshot,
    HarvestNodeEvent,
    HarvestNodeEventKind,
    HarvestNodeSnapshot,
    HarvestResult,
    InteractionAction,
    InteractionTarget,
    InventoryDelta,
    InventoryItemSnapshot,
    InventorySnapshot,
    PlayerActionSnapshot,
    PlayerSnapshot,
    ServerMsg,
    SkillDelta,
    SkillSnapshot,
    SkillSnapshotEntry,
    SkillXpSource,
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
    pub move_progress: f32,
    pub action: Option<PlayerActionSnapshot>,
}
