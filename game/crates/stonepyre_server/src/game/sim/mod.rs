pub mod world;

use self::world::{PlayerState, ServerAction, WorldState, SERVER_MOVE_TILES_PER_SEC};
use crate::game::protocol::{
    ActionState,
    InteractionAction,
    InteractionTarget,
    PlayerSnapshot,
    ServerMsg,
    WorldSnapshot,
};
use stonepyre_world::TilePos;
use uuid::Uuid;

const MAX_BFS_ITERS: usize = 50_000;
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
                action: None,
            },
        );
    }

    pub fn remove_player(&mut self, player_id: Uuid) {
        self.world.players.remove(&player_id);
    }

    /// Client asks to move somewhere.
    ///
    /// Any manual movement cancels the current queued/active non-movement action.
    pub fn set_move_target(&mut self, player_id: Uuid, requested: TilePos) -> Option<ServerMsg> {
        let start_tile = match self.world.players.get(&player_id) {
            Some(p) => p.tile,
            None => return None,
        };

        let goal = if self.world.is_blocked(requested) {
            match self
                .world
                .pick_best_adjacent_unblocked(start_tile, requested, 1)
            {
                Some(g) => g,
                None => return None,
            }
        } else {
            requested
        };

        let path = self.world.find_path_bfs(start_tile, goal, MAX_BFS_ITERS);
        let cancelled = self.clear_action(player_id, ActionState::Cancelled, "movement cancelled action");

        if let Some(p) = self.world.players.get_mut(&player_id) {
            p.goal = Some(goal);
            p.path = path;
            p.last_repath_tick = self.tick;
            p.move_progress_tiles = 0.0;
        }

        cancelled
    }

    /// Queue a server-authoritative ChopDown request.
    ///
    /// Distance is not a final rejection. If the target is valid and reachable,
    /// the server moves the player to an adjacent tile and exposes the action
    /// lifecycle through snapshots.
    pub fn queue_chop_down(&mut self, player_id: Uuid, target: TilePos) -> Result<(ActionState, String), String> {
        if !self.world.is_choppable_tree(target) {
            return Err("target is not a choppable tree".to_string());
        }

        let Some(start_tile) = self.world.players.get(&player_id).map(|p| p.tile) else {
            return Err("player is not in the world".to_string());
        };

        let distance = manhattan(start_tile, target);
        if distance <= 1 {
            if let Some(p) = self.world.players.get_mut(&player_id) {
                p.goal = None;
                p.path.clear();
                p.move_progress_tiles = 0.0;
                p.action = Some(ServerAction {
                    action: InteractionAction::ChopDown,
                    target: InteractionTarget::Tile(target),
                    state: ActionState::Active,
                });
            }

            return Ok((ActionState::Active, format!("ChopDown active at {},{}", target.x, target.y)));
        }

        let Some(goal) = self.world.pick_best_adjacent_unblocked(start_tile, target, 1) else {
            return Err("no reachable adjacent tile for ChopDown".to_string());
        };

        let path = self.world.find_path_bfs(start_tile, goal, MAX_BFS_ITERS);
        if path.is_empty() && start_tile != goal {
            return Err("no path to ChopDown target".to_string());
        }

        if let Some(p) = self.world.players.get_mut(&player_id) {
            p.goal = Some(goal);
            p.path = path;
            p.last_repath_tick = self.tick;
            p.move_progress_tiles = 0.0;
            p.action = Some(ServerAction {
                action: InteractionAction::ChopDown,
                target: InteractionTarget::Tile(target),
                state: ActionState::MovingToRange,
            });
        }

        Ok((ActionState::MovingToRange, format!(
            "ChopDown queued; walking to {},{} for target {},{}",
            goal.x, goal.y, target.x, target.y
        )))
    }

    /// Advance server simulation and return lifecycle events produced this tick.
    pub fn step(&mut self) -> Vec<ServerMsg> {
        self.tick += 1;
        let mut events = Vec::new();

        let blocked_snapshot = self.world.blocked.clone();

        for p in self.world.players.values_mut() {
            let Some(goal) = p.goal else {
                maybe_activate_action(p, &mut events);
                continue;
            };

            if p.tile == goal {
                p.goal = None;
                p.path.clear();
                p.move_progress_tiles = 0.0;
                maybe_activate_action(p, &mut events);
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
                    maybe_activate_action(p, &mut events);
                    break;
                }
            }
        }

        events
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
                    action: p.action.as_ref().map(|a| a.snapshot()),
                })
                .collect(),
        }
    }

    fn clear_action(&mut self, player_id: Uuid, state: ActionState, message: &str) -> Option<ServerMsg> {
        let p = self.world.players.get_mut(&player_id)?;
        let action = p.action.take()?;
        Some(ServerMsg::ActionState {
            player_id,
            action: action.action,
            target: action.target,
            state,
            message: message.to_string(),
        })
    }
}

fn maybe_activate_action(p: &mut PlayerState, events: &mut Vec<ServerMsg>) {
    let Some(action) = p.action.as_mut() else { return; };
    if action.state != ActionState::MovingToRange && action.state != ActionState::Queued {
        return;
    }

    match action.target.clone() {
        InteractionTarget::Tile(target) => {
            if manhattan(p.tile, target) <= 1 {
                action.state = ActionState::Active;
                events.push(ServerMsg::ActionState {
                    player_id: p.player_id,
                    action: action.action,
                    target: action.target.clone(),
                    state: ActionState::Active,
                    message: format!("{:?} active at {},{}", action.action, target.x, target.y),
                });
            }
        }
    }
}

fn manhattan(a: TilePos, b: TilePos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
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
