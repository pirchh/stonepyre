use crate::TilePos;

pub type TileId = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkPos {
    pub cx: i32,
    pub cy: i32,
}

impl ChunkPos {
    pub const fn new(cx: i32, cy: i32) -> Self {
        Self { cx, cy }
    }
}

/// v0 tile: ground only (decal/structure can be added later).
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub ground: TileId,
}

impl Default for Tile {
    fn default() -> Self {
        Self { ground: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub pos: ChunkPos,
    pub size: u32,         // e.g. 64
    pub tiles: Vec<Tile>,  // size*size
    // v0: objects stored globally in WorldGrid; later you can store per-chunk
}

impl Chunk {
    pub fn new(pos: ChunkPos, size: u32) -> Self {
        let len = (size as usize) * (size as usize);
        Self {
            pos,
            size,
            tiles: vec![Tile::default(); len],
        }
    }

    #[inline]
    pub fn index(&self, local_x: u32, local_y: u32) -> usize {
        (local_y as usize) * (self.size as usize) + (local_x as usize)
    }

    pub fn get_local(&self, local_x: u32, local_y: u32) -> Tile {
        self.tiles[self.index(local_x, local_y)]
    }

    pub fn set_local(&mut self, local_x: u32, local_y: u32, t: Tile) {
        let idx = self.index(local_x, local_y);
        self.tiles[idx] = t;
    }
}

/// Helper: world TilePos -> (ChunkPos, local coords).
pub fn world_to_chunk(tile: TilePos, chunk_size: u32) -> (ChunkPos, u32, u32) {
    // NOTE: handle negatives correctly
    let cs = chunk_size as i32;

    let cx = div_floor(tile.x, cs);
    let cy = div_floor(tile.y, cs);

    let lx = (tile.x - cx * cs) as u32;
    let ly = (tile.y - cy * cs) as u32;

    (ChunkPos::new(cx, cy), lx, ly)
}

fn div_floor(a: i32, b: i32) -> i32 {
    let mut q = a / b;
    let r = a % b;
    if (r != 0) && ((r > 0) != (b > 0)) {
        q -= 1;
    }
    q
}