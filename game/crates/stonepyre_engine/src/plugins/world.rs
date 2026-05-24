use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

use stonepyre_world::{tile_to_world3d, TilePos, WorldGrid, TILE_SIZE};

use crate::plugins::inventory::{Equipment, EquippedBackpack, Inventory, ItemStack};

pub const ARRIVE_EPS: f32 = 1.5;
pub const MOVE_TILES_PER_SEC: f32 = 1.6;

// Camera offset above/behind the player (isometric-ish, OSRS-style).
// Tweak Y (height) and Z (distance behind) to taste.
pub const CAM_OFFSET: Vec3 = Vec3::new(0.0, 350.0, 280.0);

// Scale applied to the player GLB.  Mixamo rigs export in centimetres so a
// character is ~180 units tall.  One tile = TILE_SIZE = 64 world units.
// A character ~1.5 tiles tall ≈ 96 units → scale ≈ 96 / 180 ≈ 0.53.
pub const PLAYER_SCALE: f32 = 0.53;

pub const MOVE_SPEED: f32 = TILE_SIZE * MOVE_TILES_PER_SEC;

/// Legacy 2-D constants kept for any code that still references them.
pub const FOOT_OFFSET_Y: f32 = 0.0;
pub const PLAYER_SCALE_2D: f32 = 0.18;
pub const BASE_DIR: &str = "characters/humanoid/base_greyscale";
pub const WALK_FRAMES: [u32; 4] = [1, 2, 3, 4];
pub const WALK_FPS: f32 = 5.0;

#[derive(Component)]
pub struct Player;

#[derive(Component, Clone, Debug)]
pub struct PlayerAppearance {
    pub base_sprite_root: String,
}
impl Default for PlayerAppearance {
    fn default() -> Self {
        Self { base_sprite_root: BASE_DIR.to_string() }
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

/// Marks what kind of interactable an entity is.
/// Used by the interaction system to build right-click context menus.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum InteractableKind {
    Tree,
    Npc,
    BankBooth,
    GroundItem { display_name: String },
}

/// Marker on the 3-D main camera so the follow system can find it.
#[derive(Component)]
pub struct MainCamera;

/// Legacy helper kept for compilation compatibility.
/// In 3-D the "feet" position is just XZ (Y = 0).
pub fn player_feet_world(xform: &Transform) -> Vec2 {
    Vec2::new(xform.translation.x, xform.translation.z)
}

// ---------------------------------------------------------------------------
// World spawn
// ---------------------------------------------------------------------------

pub fn spawn_demo_world_for_character(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    _harvest_defs: Option<&crate::plugins::skills::HarvestDb>,
    _character_id: Uuid,
) {
    // ------------------------------------------------------------------
    // Camera (3-D, isometric-ish)
    // ------------------------------------------------------------------
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(CAM_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
        IsDefaultUiCamera,
    ));

    // ------------------------------------------------------------------
    // Lighting
    // ------------------------------------------------------------------
    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        affects_lightmapped_meshes: false,
    });

    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));

    // ------------------------------------------------------------------
    // Ground plane
    // ------------------------------------------------------------------
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.38, 0.18),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4096.0, 4096.0))),
        MeshMaterial3d(ground_mat),
        Transform::default(),
    ));

    // ------------------------------------------------------------------
    // Seed inventory
    // ------------------------------------------------------------------
    let mut inv = Inventory::new(16);
    inv.container.slots[0] = Some(ItemStack { id: "axe_iron".to_string(),  qty: 1 });
    inv.container.slots[1] = Some(ItemStack { id: "log_oak".to_string(),   qty: 1 });
    inv.container.slots[2] = Some(ItemStack { id: "log_oak".to_string(),   qty: 1 });

    // ------------------------------------------------------------------
    // Load the player GLB and store its handle as a resource so the
    // animation setup system can access it.
    // ------------------------------------------------------------------
    let gltf_handle: Handle<bevy::gltf::Gltf> =
        asset_server.load("characters/humanoid/player.glb");
    commands.insert_resource(
        crate::plugins::animation::PlayerGltfHandle(gltf_handle.clone()),
    );
    commands.insert_resource(crate::plugins::animation::PlayerAnimGraph::default());

    // ------------------------------------------------------------------
    // Player entity — SceneRoot loads the GLB scene
    // ------------------------------------------------------------------
    commands.spawn((
        SceneRoot(asset_server.load("characters/humanoid/player.glb#Scene0")),
        Transform::from_translation(tile_to_world3d(TilePos::new(0, 0)))
            .with_scale(Vec3::splat(PLAYER_SCALE)),
        Player,
        MoveSpeed(MOVE_SPEED),
        TilePath::default(),
        Facing::South,
        crate::plugins::animation::HumanoidAnim3d::default(),
        PlayerAppearance::default(),
        inv,
        Equipment::default(),
        EquippedBackpack::default(),
    ));

    // ------------------------------------------------------------------
    // Demo NPC — blue box
    // ------------------------------------------------------------------
    let npc_tile = TilePos::new(-2, 1);
    let npc_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.4, 0.9),
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(TILE_SIZE * 0.5, TILE_SIZE * 1.5, TILE_SIZE * 0.5))),
        MeshMaterial3d(npc_mat),
        Transform::from_translation(
            tile_to_world3d(npc_tile) + Vec3::new(0.0, TILE_SIZE * 0.75, 0.0),
        ),
        GridPos(npc_tile),
        BlocksMovement,
        InteractableKind::Npc,
    ));

    // ------------------------------------------------------------------
    // Demo bank booths — gold boxes
    // ------------------------------------------------------------------
    let booth_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.7, 0.1),
        ..default()
    });
    for (x, y) in [(-4i32, 1i32), (-5, 1)] {
        let booth_tile = TilePos::new(x, y);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(TILE_SIZE * 0.9, TILE_SIZE * 1.2, TILE_SIZE * 0.9))),
            MeshMaterial3d(booth_mat.clone()),
            Transform::from_translation(
                tile_to_world3d(booth_tile) + Vec3::new(0.0, TILE_SIZE * 0.6, 0.0),
            ),
            GridPos(booth_tile),
            BlocksMovement,
            InteractableKind::BankBooth,
        ));
    }

    // ------------------------------------------------------------------
    // Target marker — flat green tile highlight
    // ------------------------------------------------------------------
    let marker_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.2, 0.8, 0.2, 0.4),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(TILE_SIZE * 0.9, TILE_SIZE * 0.9))),
        MeshMaterial3d(marker_mat),
        Transform::from_translation(Vec3::new(0.0, 0.02, 0.0)), // just above ground
        TargetMarker::default(),
        Visibility::Hidden,
    ));
}

/// Optional helper if you want a legacy call with no character id.
pub fn spawn_demo_world_legacy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    harvest_defs: Option<&crate::plugins::skills::HarvestDb>,
) {
    spawn_demo_world_for_character(commands, asset_server, meshes, materials, harvest_defs, Uuid::nil());
}

// ---------------------------------------------------------------------------
// Camera follow
// ---------------------------------------------------------------------------

pub fn camera_follow_player(
    player_q: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut cam_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player_xform) = player_q.single() else { return; };
    let Ok(mut cam_xform) = cam_q.single_mut() else { return; };

    let target = player_xform.translation + CAM_OFFSET;
    // Smooth follow — lerp toward the desired position.
    cam_xform.translation = cam_xform.translation.lerp(target, 0.12);
    cam_xform.look_at(player_xform.translation, Vec3::Y);
}

// ---------------------------------------------------------------------------
// World grid sync (unchanged)
// ---------------------------------------------------------------------------

pub fn sync_world_grid_blocked(
    world: Option<ResMut<WorldGrid>>,
    blockers: Query<&GridPos, With<BlocksMovement>>,
) {
    let Some(mut world) = world else { return; };
    let blocked: HashSet<TilePos> = blockers.iter().map(|gp| gp.0).collect();
    world.set_blocked(blocked);
}

// ---------------------------------------------------------------------------
// Target marker (adapted for 3-D)
// ---------------------------------------------------------------------------

pub fn debug_draw_target_marker(
    player_q: Query<(&TilePath, Option<&crate::plugins::movement::StepTo>), With<Player>>,
    mut marker_q: Query<(&mut Transform, &mut Visibility), With<TargetMarker>>,
) {
    let Ok((path, step)) = player_q.single() else { return; };
    let Ok((mut marker_xform, mut vis)) = marker_q.single_mut() else { return; };

    let goal = step
        .map(|s| s.0)
        .or_else(|| path.tiles.back().copied());

    if let Some(tile) = goal {
        let world = tile_to_world3d(tile);
        marker_xform.translation = Vec3::new(world.x, 0.02, world.z);
        *vis = Visibility::Visible;
    } else {
        *vis = Visibility::Hidden;
    }
}
