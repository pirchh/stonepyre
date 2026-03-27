use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{neighbors_4, TilePos};
use crate::objects::{PlacedObject, WorldObjectDef};
use crate::chunk::{Chunk, ChunkPos, Tile, world_to_chunk};
use crate::source::WorldSource;

#[derive(Resource)]
pub struct WorldGrid {
    pub chunk_size: u32,
    pub chunks: HashMap<ChunkPos, Chunk>,

    /// v0 objects live here globally.
    pub objects: Vec<PlacedObject>,

    /// v0 collision cache (keep for performance + simplicity).
    pub blocked: HashSet<TilePos>,

    /// Chunk generator / loader.
    pub source: Box<dyn WorldSource>,
}

impl WorldGrid {
    pub fn new(chunk_size: u32, source: Box<dyn WorldSource>) -> Self {
        Self {
            chunk_size,
            chunks: HashMap::new(),
            objects: Vec::new(),
            blocked: HashSet::new(),
            source,
        }
    }

    /// ✅ Compatibility bridge:
    /// Today, the engine computes blockers from ECS entities and pushes them here.
    /// Later, we'll remove this and instead derive `blocked` from `world.objects`.
    pub fn set_blocked(&mut self, tiles: HashSet<TilePos>) {
        self.blocked = tiles;
    }

    pub fn ensure_chunk_loaded(&mut self, pos: ChunkPos) {
        if self.chunks.contains_key(&pos) {
            return;
        }
        let c = self.source.generate_chunk(pos, self.chunk_size);
        self.chunks.insert(pos, c);
    }

    pub fn tile_at(&mut self, t: TilePos) -> Tile {
        let (cp, lx, ly) = world_to_chunk(t, self.chunk_size);
        self.ensure_chunk_loaded(cp);
        self.chunks.get(&cp).unwrap().get_local(lx, ly)
    }

    pub fn is_blocked(&self, t: TilePos) -> bool {
        self.blocked.contains(&t)
    }

    /// v0: brute scan objects. (Later: per-chunk storage or spatial index)
    pub fn objects_at(&self, t: TilePos) -> impl Iterator<Item = &PlacedObject> {
        self.objects.iter().filter(move |o| o.origin == t)
    }

    pub fn rebuild_blocked_cache(&mut self) {
        let mut blocked = HashSet::new();
        for o in &self.objects {
            if o.blocks_movement {
                for occ in o.occupied_tiles() {
                    blocked.insert(occ);
                }
            }
        }
        self.blocked = blocked;
    }

    /// Compatibility: populate blocked from old WorldObjectDef list.
    pub fn set_blocked_from_objects(&mut self, objects: &[WorldObjectDef]) {
        let mut blocked = HashSet::new();
        for o in objects {
            if o.blocks_movement {
                blocked.insert(o.tile);
            }
        }
        self.blocked = blocked;
    }

    /// BFS path (4-dir). Returns steps excluding `start`, including `goal`.
    pub fn find_path_bfs(&self, start: TilePos, goal: TilePos) -> VecDeque<TilePos> {
        if start == goal {
            return VecDeque::new();
        }
        if self.is_blocked(goal) {
            return VecDeque::new();
        }

        // safety bounds
        let margin: i32 = 32;
        let min_x = start.x.min(goal.x) - margin;
        let max_x = start.x.max(goal.x) + margin;
        let min_y = start.y.min(goal.y) - margin;
        let max_y = start.y.max(goal.y) + margin;

        let in_bounds =
            |t: TilePos| t.x >= min_x && t.x <= max_x && t.y >= min_y && t.y <= max_y;

        let mut frontier = VecDeque::new();
        let mut came_from: HashMap<TilePos, TilePos> = HashMap::new();
        let mut visited: HashSet<TilePos> = HashSet::new();

        frontier.push_back(start);
        visited.insert(start);

        let max_visits: usize = 40_000;

        while let Some(cur) = frontier.pop_front() {
            if cur == goal {
                break;
            }
            if visited.len() > max_visits {
                break;
            }

            for next in neighbors_4(cur) {
                if !in_bounds(next) {
                    continue;
                }
                if visited.contains(&next) {
                    continue;
                }
                if self.is_blocked(next) {
                    continue;
                }

                visited.insert(next);
                came_from.insert(next, cur);
                frontier.push_back(next);
            }
        }

        if !came_from.contains_key(&goal) {
            return VecDeque::new();
        }

        // reconstruct
        let mut rev: Vec<TilePos> = Vec::new();
        let mut cur = goal;
        rev.push(cur);

        while cur != start {
            let Some(prev) = came_from.get(&cur).copied() else { break; };
            cur = prev;
            if cur != start {
                rev.push(cur);
            }
        }

        rev.reverse();
        rev.into_iter().collect()
    }
}