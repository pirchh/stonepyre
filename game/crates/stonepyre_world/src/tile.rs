use bevy::prelude::*;
use serde::{Deserialize, Serialize}; // ✅ NEW

pub const TILE_SIZE: f32 = 64.0;

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
        y: (world.y / TILE_SIZE).round() as i32,
    }
}

pub fn tile_to_world_center(tile: TilePos) -> Vec2 {
    Vec2::new(tile.x as f32 * TILE_SIZE, tile.y as f32 * TILE_SIZE)
}

pub fn neighbors_4(tile: TilePos) -> [TilePos; 4] {
    [
        TilePos::new(tile.x + 1, tile.y),
        TilePos::new(tile.x - 1, tile.y),
        TilePos::new(tile.x, tile.y + 1),
        TilePos::new(tile.x, tile.y - 1),
    ]
}