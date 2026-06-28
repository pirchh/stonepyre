use bevy::prelude::*;
use serde::{Deserialize, Serialize}; // ✅ NEW

/// Tile width in world units.
pub const TILE_SIZE: f32 = 64.0;

/// Tile height in world units. Shorter than TILE_SIZE to give a squashed
/// top-down perspective — N/S movement covers fewer pixels than E/W,
/// creating a sense of depth without full isometric projection.
pub const TILE_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)] // ✅ NEW
pub struct TilePos {
    pub x: i32,
    pub y: i32,
}

impl TilePos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

pub fn world_to_tile(world: Vec2) -> TilePos {
    TilePos {
        x: (world.x / TILE_SIZE).round() as i32,
        y: (world.y / TILE_HEIGHT).round() as i32,
    }
}

pub fn tile_to_world_center(tile: TilePos) -> Vec2 {
    Vec2::new(tile.x as f32 * TILE_SIZE, tile.y as f32 * TILE_HEIGHT)
}

/// Convert a tile position to a 3D world position.
/// Tile X maps to world X, tile Y maps to world -Z (Y+ is North = into the screen).
/// World Y is always 0 (ground plane).
pub fn tile_to_world3d(tile: TilePos) -> Vec3 {
    Vec3::new(tile.x as f32 * TILE_SIZE, 0.0, -(tile.y as f32 * TILE_SIZE))
}

/// Canonical world-pos `[x, z]` → tile. The SINGLE rounding rule shared by the
/// client predictor, the server sim, and client replay reconciliation — route
/// all movement/collision tile lookups through this so prediction and authority
/// agree bit-for-bit. (Previously the server used `floor(x + half)` while the
/// client used `round`, disagreeing at negative half-tile boundaries.)
pub fn world_pos_to_tile(pos: [f32; 2]) -> TilePos {
    TilePos {
        x: (pos[0] / TILE_SIZE).round() as i32,
        y: -(pos[1] / TILE_SIZE).round() as i32,
    }
}

/// Convert a 3D world position back to the nearest tile.
pub fn world3d_to_tile(world: Vec3) -> TilePos {
    world_pos_to_tile([world.x, world.z])
}

/// Deterministic collision-aware step: try the full move, then slide along X,
/// then along Z, returning the furthest unblocked position. `is_blocked` is the
/// caller's collision predicate (the server's `HashSet`, the client's
/// `WorldGrid`). Shared so client prediction and server simulation produce
/// identical results for the same input and blockers — the basis for replay.
pub fn slide_move(pos: [f32; 2], delta: [f32; 2], is_blocked: impl Fn(TilePos) -> bool) -> [f32; 2] {
    let blocked = |p: [f32; 2]| is_blocked(world_pos_to_tile(p));
    let full = [pos[0] + delta[0], pos[1] + delta[1]];
    if !blocked(full) {
        return full;
    }
    let slide_x = [pos[0] + delta[0], pos[1]];
    if !blocked(slide_x) {
        return slide_x;
    }
    let slide_z = [pos[0], pos[1] + delta[1]];
    if !blocked(slide_z) {
        return slide_z;
    }
    pos
}

pub fn neighbors_4(tile: TilePos) -> [TilePos; 4] {
    [
        TilePos::new(tile.x + 1, tile.y),
        TilePos::new(tile.x - 1, tile.y),
        TilePos::new(tile.x, tile.y + 1),
        TilePos::new(tile.x, tile.y - 1),
    ]
}