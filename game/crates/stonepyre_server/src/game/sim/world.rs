use std::collections::{HashMap, HashSet, VecDeque};

use stonepyre_world::{neighbors_4, TilePos};
use uuid::Uuid;

/// The server's world snapshot state (v0).
/// - `blocked` is your "unwalkable tiles" set (water, cliffs, etc.)
/// - Later: expand this to include object/building footprints.
pub struct WorldState {
    pub players: HashMap<Uuid, PlayerState>,
    pub blocked: HashSet<TilePos>,
}

impl WorldState {
    pub fn new() -> Self {
        let mut blocked = HashSet::new();

        // Demo unwalkable tiles (water). Replace with real tilemap logic later.
        // A little vertical wall at x=3 from y=-2..=2
        for y in -2..=2 {
            blocked.insert(TilePos::new(3, y));
        }

        Self {
            players: HashMap::new(),
            blocked,
        }
    }

    #[inline]
    pub fn is_blocked(&self, t: TilePos) -> bool {
        self.blocked.contains(&t)
    }

    /// If the requested target tile is blocked, try to select a nearby
    /// unblocked tile within `range` using a cheap heuristic (closest to start).
    pub fn pick_best_adjacent_unblocked(
        &self,
        start: TilePos,
        target: TilePos,
        range: i32,
    ) -> Option<TilePos> {
        let mut best: Option<TilePos> = None;
        let mut best_score = i32::MAX;

        for dx in -range..=range {
            for dy in -range..=range {
                // manhattan ring-ish
                if dx.abs() + dy.abs() > range {
                    continue;
                }
                if dx == 0 && dy == 0 {
                    continue;
                }

                let cand = TilePos::new(target.x + dx, target.y + dy);

                if self.is_blocked(cand) {
                    continue;
                }

                let dist = (cand.x - start.x).abs() + (cand.y - start.y).abs();
                if dist < best_score {
                    best_score = dist;
                    best = Some(cand);
                }
            }
        }

        best
    }

    /// BFS pathfind on a 4-neighbor grid avoiding `blocked`.
    /// Returns a path of steps from start -> goal (excluding start).
    pub fn find_path_bfs(&self, start: TilePos, goal: TilePos, max_iters: usize) -> VecDeque<TilePos> {
        if start == goal {
            return VecDeque::new();
        }
        if self.is_blocked(goal) {
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
                if self.is_blocked(n) {
                    continue;
                }

                visited.insert(n);
                came_from.insert(n, cur);

                if n == goal {
                    // reconstruct
                    return reconstruct_path(start, goal, &came_from);
                }

                q.push_back(n);
            }
        }

        VecDeque::new()
    }
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
            // no path
            return VecDeque::new();
        }
    }

    out
}

pub struct PlayerState {
    pub player_id: Uuid,
    pub character_id: Uuid,

    /// Current tile pos
    pub tile: TilePos,

    /// Desired destination tile (unblocked goal after adjustment)
    pub goal: Option<TilePos>,

    /// Current computed path (steps from current tile -> goal)
    pub path: VecDeque<TilePos>,

    /// Used to rate-limit repathing attempts
    pub last_repath_tick: u64,
}\