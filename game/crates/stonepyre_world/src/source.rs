use crate::chunk::{Chunk, ChunkPos, Tile};
use crate::chunk::TileId;

pub trait WorldSource: Send + Sync {
    fn world_seed(&self) -> u64;

    /// Generate the *base* chunk (no player deltas applied).
    fn generate_chunk(&self, pos: ChunkPos, chunk_size: u32) -> Chunk;
}

/// v0: everything is one ground tile id.
pub struct FlatWorldSource {
    pub seed: u64,
    pub ground: TileId,
}

impl FlatWorldSource {
    pub fn new(seed: u64, ground: TileId) -> Self {
        Self { seed, ground }
    }
}

impl WorldSource for FlatWorldSource {
    fn world_seed(&self) -> u64 {
        self.seed
    }

    fn generate_chunk(&self, pos: ChunkPos, chunk_size: u32) -> Chunk {
        let mut c = Chunk::new(pos, chunk_size);
        let len = (chunk_size as usize) * (chunk_size as usize);
        c.tiles = vec![Tile { ground: self.ground }; len];
        c
    }
}