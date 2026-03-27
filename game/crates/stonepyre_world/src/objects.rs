use crate::TilePos;

pub type ObjectId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorldObjectKind {
    Tree,
    Npc,
    // Later: Rock, Door, Chest, Crop, Wall, etc.
}

#[derive(Clone, Debug)]
pub enum ObjectState {
    None,
    // Later you can evolve this into structured state
    // (e.g., growth stage, charges, regen timers, etc.)
}

impl Default for ObjectState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug)]
pub struct PlacedObject {
    pub id: ObjectId,
    pub kind: WorldObjectKind,
    /// Origin tile (top-left if footprint > 1x1 later).
    pub origin: TilePos,
    pub blocks_movement: bool,
    pub state: ObjectState,
}

impl PlacedObject {
    pub fn new(id: ObjectId, kind: WorldObjectKind, origin: TilePos, blocks_movement: bool) -> Self {
        Self {
            id,
            kind,
            origin,
            blocks_movement,
            state: ObjectState::None,
        }
    }

    /// v0 footprint is always 1x1.
    /// Later: return iterator over occupied tiles from footprint + origin.
    pub fn occupied_tiles(&self) -> [TilePos; 1] {
        [self.origin]
    }
}

/// Keeping your old name so you don't break any engine imports.
#[derive(Clone, Copy, Debug)]
pub struct WorldObjectDef {
    pub kind: WorldObjectKind,
    pub tile: TilePos,
    pub blocks_movement: bool,
}

impl WorldObjectDef {
    pub const fn new(kind: WorldObjectKind, tile: TilePos, blocks_movement: bool) -> Self {
        Self { kind, tile, blocks_movement }
    }
}

/// Demo world objects (authoritative source of truth for your test scene).
pub fn demo_objects() -> Vec<WorldObjectDef> {
    vec![
        WorldObjectDef::new(WorldObjectKind::Tree, TilePos::new(2, 0), true),
        WorldObjectDef::new(WorldObjectKind::Npc,  TilePos::new(-2, 1), true),
    ]
}