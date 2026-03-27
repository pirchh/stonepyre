pub mod world;

use self::world::{PlayerState, WorldState};
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
}

impl GameSim {
    pub fn new() -> Self {
        Self {
            tick: 0,
            world: WorldState::new(),
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
            },
        );
    }

    pub fn remove_player(&mut self, player_id: Uuid) {
        self.world.players.remove(&player_id);
    }

    /// Client asks to move somewhere.
    /// Server adjusts goal if blocked, computes path, stores it.
    pub fn set_move_target(&mut self, player_id: Uuid, requested: TilePos) {
        // Extract current tile first to avoid borrow conflicts.
        let start_tile = match self.world.players.get(&player_id) {
            Some(p) => p.tile,
            None => return,
        };

        // If requested is blocked, pick best nearby.
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
            p.last_repath_tick = self.tick; // treat this as a repath too
        }
    }

    pub fn step(&mut self) {
        self.tick += 1;

        // Clone blocked once so we can consult it while mutating players without borrow checker fights.
        // (Still 100% server-authoritative.)
        let blocked_snapshot = self.world.blocked.clone();

        for p in self.world.players.values_mut() {
            // If we have no goal, nothing to do.
            let Some(goal) = p.goal else { continue; };

            // If we're already there, clear goal/path.
            if p.tile == goal {
                p.goal = None;
                p.path.clear();
                continue;
            }

            // If we don't currently have a path but still have a goal,
            // attempt to compute a new one (rate-limited).
            if p.path.is_empty() {
                if (self.tick - p.last_repath_tick) >= REPATH_COOLDOWN_TICKS {
                    p.last_repath_tick = self.tick;

                    // IMPORTANT: we can't call self.world.find_path_bfs here because of borrows,
                    // so we compute with a helper that uses the cloned blocked set.
                    p.path = find_path_bfs_with_blocked(p.tile, goal, &blocked_snapshot, MAX_BFS_ITERS);
                }

                // Still no path? Wait.
                if p.path.is_empty() {
                    continue;
                }
            }

            // We have a path step; validate next tile isn't blocked.
            let next = *p.path.front().unwrap();

            if blocked_snapshot.contains(&next) {
                // Next step became blocked mid-walk -> clear and repath (rate-limited).
                p.path.clear();

                if (self.tick - p.last_repath_tick) >= REPATH_COOLDOWN_TICKS {
                    p.last_repath_tick = self.tick;
                    p.path = find_path_bfs_with_blocked(p.tile, goal, &blocked_snapshot, MAX_BFS_ITERS);
                }

                continue;
            }

            // Apply one step per tick.
            p.tile = next;
            p.path.pop_front();
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
                })
                .collect(),
        }
    }
}

// --- Local BFS helper avoiding borrow issues (uses blocked snapshot) ---

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