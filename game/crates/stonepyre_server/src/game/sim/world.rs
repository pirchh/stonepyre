use std::collections::{HashMap, HashSet, VecDeque};

use stonepyre_world::{neighbors_4, TilePos, TILE_SIZE};
use uuid::Uuid;

use super::harvest::{HarvestCatalog, HarvestLootPreview, HarvestNodeDef, HarvestSkill};
use crate::game::protocol::{
    ActionState,
    GroundItemEvent,
    GroundItemEventKind,
    GroundItemSnapshot,
    InteractionAction,
    InteractionTarget,
    PlayerActionSnapshot,
};

/// Server-side movement speed in tiles/sec (kept for harvest walk-to-range).
pub const SERVER_MOVE_TILES_PER_SEC: f32 = 1.6;

/// World-units per second for continuous WASD movement.
/// Must match stonepyre_engine::plugins::world::MOVE_SPEED (TILE_SIZE * 1.6).
pub const SERVER_MOVE_SPEED: f32 = TILE_SIZE * SERVER_MOVE_TILES_PER_SEC;

// ---------------------------------------------------------------------------
// Continuous movement helpers
// ---------------------------------------------------------------------------

/// Convert a world-space Vec2(x, z) to the tile it sits in.
pub fn pos_to_tile(pos: [f32; 2]) -> TilePos {
    let half = TILE_SIZE * 0.5;
    TilePos {
        x: ((pos[0] + half) / TILE_SIZE).floor() as i32,
        y: ((pos[1] + half) / TILE_SIZE).floor() as i32,
    }
}

/// Convert tile centre to world pos [x, z].
pub fn tile_to_pos(tile: TilePos) -> [f32; 2] {
    [tile.x as f32 * TILE_SIZE, tile.y as f32 * TILE_SIZE]
}

/// Try to move from `pos` by `delta`, sliding along walls.
/// Returns the furthest unblocked position.
pub fn try_move_continuous(pos: [f32; 2], delta: [f32; 2], blocked: &HashSet<TilePos>) -> [f32; 2] {
    let full = [pos[0] + delta[0], pos[1] + delta[1]];
    if !pos_blocked(full, blocked) {
        return full;
    }
    let slide_x = [pos[0] + delta[0], pos[1]];
    if !pos_blocked(slide_x, blocked) {
        return slide_x;
    }
    let slide_z = [pos[0], pos[1] + delta[1]];
    if !pos_blocked(slide_z, blocked) {
        return slide_z;
    }
    pos
}

fn pos_blocked(pos: [f32; 2], blocked: &HashSet<TilePos>) -> bool {
    blocked.contains(&pos_to_tile(pos))
}

pub const GROUND_ITEM_DESPAWN_TICKS: u64 = 300;

/// The server's world snapshot state (v0).
/// - `blocked` is the authoritative unwalkable tile set.
/// - Keep this aligned with the demo-world blockers until world data becomes shared/loaded.
pub struct WorldState {
    pub players: HashMap<Uuid, PlayerState>,
    pub blocked: HashSet<TilePos>,
    pub harvest: HarvestCatalog,
    pub ground_items: HashMap<Uuid, GroundItemState>,
}

impl WorldState {
    pub fn new() -> Self {
        let harvest = HarvestCatalog::demo();
        let mut blocked = HashSet::new();

        // Harvest nodes are physical world objects, so their tiles are blocked.
        // v0 keeps these in memory; later this can come from loaded world/object data.
        blocked.extend(harvest.blocking_tiles());

        // Match the current client demo NPC blocker until world data becomes shared/loaded.
        blocked.insert(TilePos::new(-2, 1));

        Self {
            players: HashMap::new(),
            blocked,
            harvest,
            ground_items: HashMap::new(),
        }
    }

    #[inline]
    pub fn is_blocked(&self, t: TilePos) -> bool {
        self.blocked.contains(&t)
    }

    #[inline]
    pub fn harvest_node_def_at(&self, t: TilePos) -> Option<&HarvestNodeDef> {
        self.harvest.node_def_at(t)
    }

    #[inline]
    pub fn harvest_loot_preview_at(&self, t: TilePos) -> Option<HarvestLootPreview> {
        self.harvest.loot_preview_at(t)
    }

    #[inline]
    pub fn is_choppable_tree(&self, t: TilePos) -> bool {
        self.harvest_node_def_at(t)
            .map(|def| def.skill == HarvestSkill::Woodcutting)
            .unwrap_or(false)
            && self.world_harvest_node_ready(t)
    }

    #[inline]
    fn world_harvest_node_ready(&self, t: TilePos) -> bool {
        self.harvest.can_harvest_at(t)
    }

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
                    return reconstruct_path(start, goal, &came_from);
                }

                q.push_back(n);
            }
        }

        VecDeque::new()
    }

    pub fn spawn_ground_item(
        &mut self,
        item_id: String,
        quantity: u32,
        tile: TilePos,
        owner_character_id: Option<Uuid>,
        current_tick: u64,
    ) -> GroundItemEvent {
        let ground_item = GroundItemState {
            ground_item_id: Uuid::new_v4(),
            item_id,
            quantity,
            tile,
            owner_character_id,
            despawn_at_tick: current_tick + GROUND_ITEM_DESPAWN_TICKS,
        };

        let snapshot = ground_item.snapshot();
        let ground_item_id = ground_item.ground_item_id;
        self.ground_items.insert(ground_item_id, ground_item);

        GroundItemEvent {
            kind: GroundItemEventKind::Spawned,
            item: Some(snapshot),
            ground_item_id,
        }
    }

    pub fn take_ground_item(
        &mut self,
        ground_item_id: Uuid,
        character_id: Uuid,
        player_tile: TilePos,
    ) -> Result<GroundItemState, String> {
        let Some(item) = self.ground_items.get(&ground_item_id) else {
            return Err("ground item no longer exists".to_string());
        };

        if let Some(owner_character_id) = item.owner_character_id {
            if owner_character_id != character_id {
                return Err("ground item does not belong to this character yet".to_string());
            }
        }

        if manhattan(player_tile, item.tile) > 1 {
            return Err("too far away to pick up item".to_string());
        }

        self.ground_items
            .remove(&ground_item_id)
            .ok_or_else(|| "ground item no longer exists".to_string())
    }

    pub fn restore_ground_item(&mut self, item: GroundItemState) {
        self.ground_items.insert(item.ground_item_id, item);
    }

    pub fn visible_ground_item_snapshots(&self) -> Vec<GroundItemSnapshot> {
        self.ground_items
            .values()
            .map(GroundItemState::snapshot)
            .collect()
    }

    pub fn tick_ground_item_despawns(&mut self, current_tick: u64) -> Vec<GroundItemEvent> {
        let expired: Vec<Uuid> = self
            .ground_items
            .iter()
            .filter_map(|(id, item)| (current_tick >= item.despawn_at_tick).then_some(*id))
            .collect();

        expired
            .into_iter()
            .filter_map(|ground_item_id| {
                self.ground_items.remove(&ground_item_id).map(|_| GroundItemEvent {
                    kind: GroundItemEventKind::Despawned,
                    item: None,
                    ground_item_id,
                })
            })
            .collect()
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
            return VecDeque::new();
        }
    }

    out
}

#[derive(Clone, Debug)]
pub struct GroundItemState {
    pub ground_item_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
    pub tile: TilePos,
    pub owner_character_id: Option<Uuid>,
    pub despawn_at_tick: u64,
}

impl GroundItemState {
    pub fn snapshot(&self) -> GroundItemSnapshot {
        GroundItemSnapshot {
            ground_item_id: self.ground_item_id,
            item_id: self.item_id.clone(),
            quantity: self.quantity,
            tile: self.tile,
            owner_character_id: self.owner_character_id,
            despawn_at_tick: self.despawn_at_tick,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServerAction {
    pub action: InteractionAction,
    pub target: InteractionTarget,
    pub state: ActionState,
    pub next_harvest_tick: Option<u64>,

    /// True while the server has asked the async DB layer whether expected
    /// harvest loot can fit before allowing this action to become Active.
    pub pending_harvest_capacity_check: bool,
}

impl ServerAction {
    pub fn snapshot(&self) -> PlayerActionSnapshot {
        PlayerActionSnapshot {
            action: self.action,
            target: self.target.clone(),
            state: self.state,
        }
    }
}

pub struct PlayerState {
    pub player_id: Uuid,
    pub character_id: Uuid,

    // ------------------------------------------------------------------
    // Continuous WASD position (primary for rendering)
    // ------------------------------------------------------------------

    /// Authoritative world-space position [x, z] in world units.
    /// Updated every tick from `move_dir`.
    pub pos: [f32; 2],

    /// Normalised movement direction received from the client this tick.
    /// `[0, 0]` means the player is standing still.
    pub move_dir: [f32; 2],

    // ------------------------------------------------------------------
    // Tile-based state (derived from pos; used for interaction checks)
    // ------------------------------------------------------------------

    /// Derived tile — `pos_to_tile(pos)` computed each tick.
    pub tile: TilePos,

    /// Desired destination tile for harvest walk-to-range movement.
    pub goal: Option<TilePos>,

    /// Current computed path (steps from current tile -> goal).
    pub path: VecDeque<TilePos>,

    /// Used to rate-limit repathing attempts.
    pub last_repath_tick: u64,

    /// Fractional movement accumulator in tiles (harvest walk-to-range only).
    pub move_progress_tiles: f32,

    /// Current server-owned non-movement action lifecycle.
    pub action: Option<ServerAction>,
}

fn manhattan(a: TilePos, b: TilePos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
