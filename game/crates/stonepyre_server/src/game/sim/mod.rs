pub mod world;

use self::world::{PlayerState, WorldState, SERVER_MOVE_TILES_PER_SEC};
use crate::game::protocol::{PlayerSnapshot, WorldSnapshot};
use stonepyre_world::TilePos;
use uuid::Uuid;

const MAX_BFS_ITERS: usize = 50_000;

// If a step becomes blocked, we re-path — but not more than once every N ticks
// (prevents spamming BFS if destination is impossible).
const REPATH_COOLDOWN_TICKS: u64 = 3;

pub struct GameSim {
    pub tick: u64,
    pub world: WorldState,
    tiles_per_tick: f32,
}

impl GameSim {
    pub fn new(tick_hz: u32) -> Self {
        let tick_hz = tick_hz.max(1) as f32;
        Self {
            tick: 0,
            world: WorldState::new(),
            tiles_per_tick: SERVER_MOVE_TILES_PER_SEC / tick_hz,
        }
    }

    pub fn add_player(&mut self, player_id: Uuid, character_id: Uuid) {
        let spawn = TilePos::new(0, 0);

        self.world.players.insert(
            player_id,
            PlayerState {
                player_id,
                character_id,
                tile: spawn,
                goal: None,
                path: Default::default(),
                last_repath_tick: 0,
                move_progress_tiles: 0.0,
            },
        );
    }

    pub fn remove_player(&mut self, player_id: Uuid) {
        self.world.players.remove(&player_id);
    }

    /// Client asks to move somewhere.
    /// Server adjusts goal if blocked, computes path, stores it.
    pub fn set_move_target(&mut self, player_id: Uuid, requested: TilePos) {
        let start_tile = match self.world.players.get(&player_id) {
            Some(p) => p.tile,
            None => return,
        };

        let goal = if self.world.is_blocked(requested) {
            match self
                .world
                .pick_best_adjacent_unblocked(start_tile, requested, 1)
            {
                Some(g) => g,
                None => return,
            }
        } else {
            requested
        };

        let path = self.world.find_path_bfs(start_tile, goal, MAX_BFS_ITERS);

        if let Some(p) = self.world.players.get_mut(&player_id) {
            p.goal = Some(goal);
            p.path = path;
            p.last_repath_tick = self.tick;
            p.move_progress_tiles = 0.0;
        }
    }

    pub fn step(&mut self) {
        self.tick += 1;

        // Clone blocked once so we can consult it while mutating players without borrow checker fights.
        let blocked_snapshot = self.world.blocked.clone();

        for p in self.world.players.values_mut() {
            let Some(goal) = p.goal else { continue; };

            if p.tile == goal {
                p.goal = None;
                p.path.clear();
                p.move_progress_tiles = 0.0;
                continue;
            }

            if p.path.is_empty() {
                if (self.tick - p.last_repath_tick) >= REPATH_COOLDOWN_TICKS {
                    p.last_repath_tick = self.tick;
                    p.path = find_path_bfs_with_blocked(p.tile, goal, &blocked_snapshot, MAX_BFS_ITERS);
                }

                if p.path.is_empty() {
                    continue;
                }
            }

            p.move_progress_tiles += self.tiles_per_tick;

            // Advance whole tile steps only when enough movement time has accumulated.
            while p.move_progress_tiles >= 1.0 {
                let Some(next) = p.path.front().copied() else {
                    p.move_progress_tiles = 0.0;
                    break;
                };

                if blocked_snapshot.contains(&next) {
                    p.path.clear();
                    p.move_progress_tiles = 0.0;

                    if (self.tick - p.last_repath_tick) >= REPATH_COOLDOWN_TICKS {
                        p.last_repath_tick = self.tick;
                        p.path = find_path_bfs_with_blocked(p.tile, goal, &blocked_snapshot, MAX_BFS_ITERS);
                    }

                    break;
                }

                p.tile = next;
                p.path.pop_front();
                p.move_progress_tiles -= 1.0;

                if p.tile == goal {
                    p.goal = None;
                    p.path.clear();
                    p.move_progress_tiles = 0.0;
                    break;
                }
            }
        }
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            server_tick: self.tick,
            players: self
                .world
                .players
                .values()
                .map(|p| PlayerSnapshot {
                    player_id: p.player_id,
                    character_id: p.character_id,
                    tile: p.tile,
                    next_tile: p.path.front().copied(),
                    goal: p.goal,
                    moving: p.goal.is_some() && (!p.path.is_empty() || p.tile != p.goal.unwrap_or(p.tile)),
                })
                .collect(),
        }
    }
}

use std::collections::{HashMap, HashSet, VecDeque};
use stonepyre_world::neighbors_4;

fn find_path_bfs_with_blocked(
    start: TilePos,
    goal: TilePos,
    blocked: &HashSet<TilePos>,
    max_iters: usize,
) -> VecDeque<TilePos> {
    if start == goal {
        return VecDeque::new();
    }
    if blocked.contains(&goal) {
        return VecDeque::new();
    }

    let mut q: VecDeque<TilePos> = VecDeque::new();
    let mut came_from: HashMap<TilePos, TilePos> = HashMap::new();
    let mut visited: HashSet<TilePos> = HashSet::new();

    q.push_back(start);
    visited.insert(start);

    let mut iters = 0usize;

    while let Some(cur) = q.pop_front() {
        iters += 1;
        if iters > max_iters {
            break;
        }

        for n in neighbors_4(cur) {
            if visited.contains(&n) {
                continue;
            }
            if blocked.contains(&n) {
                continue;
            }

            visited.insert(n);
            came_from.insert(n, cur);

            if n == goal {
                return reconstruct_path(start, goal, &came_from);
            }

            q.push_back(n);
        }
    }

    VecDeque::new()
}

fn reconstruct_path(
    start: TilePos,
    goal: TilePos,
    came_from: &HashMap<TilePos, TilePos>,
) -> VecDeque<TilePos> {
    let mut out: VecDeque<TilePos> = VecDeque::new();

    let mut cur = goal;
    while cur != start {
        out.push_front(cur);
        if let Some(prev) = came_from.get(&cur) {
            cur = *prev;
        } else {
            return VecDeque::new();
        }
    }

    out
}
