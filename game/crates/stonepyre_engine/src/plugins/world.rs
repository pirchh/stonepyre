use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

use stonepyre_world::{tile_to_world_center, TilePos, WorldGrid, TILE_SIZE};

use crate::plugins::inventory::{Equipment, EquippedBackpack, Inventory, ItemStack};

pub const ARRIVE_EPS: f32 = 1.5;
pub const WALK_FPS: f32 = 5.0;
pub const PLAYER_SCALE: f32 = 0.18;
pub const MOVE_TILES_PER_SEC: f32 = 1.6;
pub const FOOT_OFFSET_Y: f32 = 30.0;

pub const BASE_DIR: &str = "characters/humanoid/base_greyscale";
pub const WALK_FRAMES: [u32; 4] = [1, 2, 3, 4];

#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Debug)]
pub struct PlayerAppearance {
    /// Root folder used by the humanoid animation loader.
    /// Example: "characters/humanoid/base_greyscale"
    pub base_sprite_root: String,
}

impl Default for PlayerAppearance {
    fn default() -> Self {
        Self {
            base_sprite_root: BASE_DIR.to_string(),
        }
    }
}

#[derive(Component)]
pub struct MoveSpeed(pub f32);

#[derive(Component, Default)]
pub struct TilePath {
    pub tiles: VecDeque<TilePos>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Facing {
    North,
    East,
    South,
    West,
}

#[derive(Component, Default)]
pub struct TargetMarker;

#[derive(Component, Clone, Copy, Debug)]
pub struct GridPos(pub TilePos);

#[derive(Component)]
pub struct BlocksMovement;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractableKind {
    Tree,
    Npc,
}

pub fn player_feet_world(xform: &Transform) -> Vec2 {
    Vec2::new(xform.translation.x, xform.translation.y - FOOT_OFFSET_Y)
}

/// Spawn the demo world + camera for a selected character.
/// Called by the app on entering AppMode::InWorld.
pub fn spawn_demo_world_for_character(
    commands: &mut Commands,
    asset_server: &AssetServer,
    harvest_defs: Option<&crate::plugins::skills::HarvestDb>,
    _character_id: Uuid,
) {
    commands.spawn(Camera2d);

    // preload humanoid frames
    let humanoid_frames = crate::plugins::animation::HumanoidFrames::load(asset_server);
    let idle_south = humanoid_frames.idle_for(Facing::South);
    commands.insert_resource(humanoid_frames);

    let move_speed = TILE_SIZE * MOVE_TILES_PER_SEC;

    // ✅ NEW: seed inventory with a couple starter items so UI can show something.
    let mut inv = Inventory::new(16);

    // "axe_iron" is just an example id. It doesn't need to exist in content yet for UI display.
    inv.container.slots[0] = Some(ItemStack {
        id: "axe_iron".to_string(),
        qty: 1,
    });

    // Demonstrate non-stackable-like behavior (same id in multiple slots).
    inv.container.slots[1] = Some(ItemStack {
        id: "log_oak".to_string(),
        qty: 1,
    });
    inv.container.slots[2] = Some(ItemStack {
        id: "log_oak".to_string(),
        qty: 1,
    });

    commands.spawn((
        Sprite {
            image: idle_south,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0 + FOOT_OFFSET_Y, 10.0).with_scale(Vec3::splat(PLAYER_SCALE)),
        Player,
        MoveSpeed(move_speed),
        TilePath::default(),
        Facing::South,
        crate::plugins::animation::HumanoidAnim::new(),
        // ✅ NEW: appearance source-of-truth for UI paperdoll + future character creator
        PlayerAppearance::default(),
        // ✅ Inventory/Equipment scaffolding
        inv,
        Equipment::default(),
        EquippedBackpack::default(),
    ));

    // Demo tree: spawn as HarvestNode(def="oak_tree") if defs exist
    if let Some(defs) = harvest_defs {
        let node = crate::plugins::skills::harvest::HarvestNode::from_def_id("oak_tree", &defs.0);

        commands.spawn((
            Sprite::from_color(Color::srgb(0.2, 0.8, 0.2), Vec2::splat(TILE_SIZE)),
            Transform::from_xyz(2.0 * TILE_SIZE, 0.0, 5.0),
            GridPos(TilePos::new(2, 0)),
            BlocksMovement,
            InteractableKind::Tree,
            Visibility::Visible,
            node,
        ));
    } else {
        commands.spawn((
            Sprite::from_color(Color::srgb(0.2, 0.8, 0.2), Vec2::splat(TILE_SIZE)),
            Transform::from_xyz(2.0 * TILE_SIZE, 0.0, 5.0),
            GridPos(TilePos::new(2, 0)),
            BlocksMovement,
            InteractableKind::Tree,
            Visibility::Visible,
        ));
    }

    // Demo NPC (blue)
    commands.spawn((
        Sprite::from_color(Color::srgb(0.2, 0.4, 0.9), Vec2::splat(TILE_SIZE)),
        Transform::from_xyz(-2.0 * TILE_SIZE, 1.0 * TILE_SIZE, 5.0),
        GridPos(TilePos::new(-2, 1)),
        BlocksMovement,
        InteractableKind::Npc,
    ));

    // Target marker fills the whole tile
    commands.spawn((
        Sprite::from_color(Color::srgba(0.2, 0.8, 0.2, 0.25), Vec2::splat(TILE_SIZE)),
        Transform::from_xyz(0.0, 0.0, 9.0),
        TargetMarker::default(),
        Visibility::Hidden,
    ));
}

/// Optional helper if you want a legacy call with no character id.
pub fn spawn_demo_world_legacy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    harvest_defs: Option<&crate::plugins::skills::HarvestDb>,
) {
    spawn_demo_world_for_character(commands, asset_server, harvest_defs, Uuid::nil());
}

pub fn sync_world_grid_blocked(
    world: Option<ResMut<WorldGrid>>,
    blockers: Query<&GridPos, With<BlocksMovement>>,
) {
    let Some(mut world) = world else {
        return;
    };

    let mut blocked = HashSet::new();
    for gp in blockers.iter() {
        blocked.insert(gp.0);
    }
    world.set_blocked(blocked);
}

pub fn debug_draw_target_marker(
    mut marker_q: Query<(&mut Transform, &mut Visibility), With<TargetMarker>>,
    player_q: Query<&TilePath, With<Player>>,
) {
    let Ok((mut mxform, mut vis)) = marker_q.single_mut() else {
        return;
    };
    let Ok(path) = player_q.single() else {
        return;
    };

    let Some(last) = path.tiles.back().copied() else {
        *vis = Visibility::Hidden;
        return;
    };

    let w = tile_to_world_center(last);
    mxform.translation.x = w.x;
    mxform.translation.y = w.y;
    *vis = Visibility::Visible;
}