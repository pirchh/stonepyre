use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

use stonepyre_world::{tile_to_world3d, TilePos, WorldGrid, TILE_SIZE};

use crate::plugins::inventory::{Equipment, EquippedBackpack, Inventory, ItemStack};
use crate::plugins::movement::IsWalking;

pub const ARRIVE_EPS: f32 = 1.5;
pub const MOVE_TILES_PER_SEC: f32 = 1.6;

// ---------------------------------------------------------------------------
// Camera rig
// ---------------------------------------------------------------------------

/// Base arm length matching the original CAM_OFFSET = Vec3(0, 350, 280).
/// sqrt(350² + 280²) ≈ 448. At zoom=1 and pitch≈51° this reproduces the
/// original camera position exactly.
const CAM_BASE_ARM: f32 = 448.0;
const ZOOM_MIN: f32 = 1.0;
const ZOOM_MAX: f32 = 4.0;
const PITCH_MIN_DEG: f32 = 25.0;
const PITCH_MAX_DEG: f32 = 70.0;
/// atan2(350, 280) ≈ 51.3° — preserves original camera angle as default.
const PITCH_DEFAULT_DEG: f32 = 51.3;

/// Drives the camera arm length (zoom) and vertical angle (pitch).
/// Scroll wheel → zoom. Ctrl + scroll wheel → pitch.
#[derive(Resource, Clone, Debug)]
pub struct CameraRig {
    /// Arm length multiplier. 1.0 = default distance, 4.0 = fully zoomed out.
    pub zoom: f32,
    /// Camera elevation angle in degrees. 25° = side-on, 70° = top-down.
    pub pitch_deg: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pitch_deg: PITCH_DEFAULT_DEG,
        }
    }
}

// Scale applied to the player GLB.
// Blender exports GLB in metres (1 Blender unit = 1 m with Apply Units on).
// A Mixamo character is ~1.8 m tall → 1.8 GLB units.
// One tile = TILE_SIZE = 64 world units; we want the player ~1.5 tiles tall ≈ 96 units.
// scale = 96 / 1.8 ≈ 53.3
// If your export used "Export All Actions" WITHOUT Apply Units, the model may
// already be ~180 units tall; in that case set this back to 0.53.
pub const PLAYER_SCALE: f32 = 53.3;

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
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

/// Source-of-truth XZ position for the player, immune to animation root-motion.
///
/// Bevy's animation system runs in PostUpdate and can overwrite `Transform.translation`
/// via root-motion curves baked into walk/idle clips. Because movement runs in Update
/// (incrementally adding to `Transform`), the animation PostUpdate reset wins and the
/// player gets stuck mid-step forever.
///
/// `LogicalPos2d` stores the movement system's intended (x, z) and is re-applied to
/// `Transform` in a PostUpdate system that runs after animation — so the final
/// per-frame transform is always the movement-owned position, not the animation's.
///
/// Stored as `Vec2(x, z)` — the `z` world axis is in `y`.
#[derive(Component, Default, Clone, Copy)]
pub struct LogicalPos2d(pub Vec2);

#[derive(Component, Default)]
pub struct TargetMarker;

#[derive(Component, Clone, Copy, Debug)]
pub struct GridPos(pub TilePos);

#[derive(Component)]
pub struct BlocksMovement;

/// Marks what kind of interactable an entity is.
/// Used by the interaction system to build right-click context menus
/// and proximity prompts.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum InteractableKind {
    Tree,
    Npc,
    BankBooth,
    GroundItem { display_name: String },
}

impl InteractableKind {
    /// Short prompt shown to the player when they are within interaction range.
    pub fn proximity_prompt(&self) -> &'static str {
        match self {
            InteractableKind::BankBooth          => "Press E to use Bank",
            InteractableKind::Npc                => "Press E to Talk",
            InteractableKind::Tree               => "Press Space to Chop",
            InteractableKind::GroundItem { .. }  => "Press E to Take",
        }
    }
}

/// Tracks the closest interactable within range of the player.
/// Updated each frame by `update_nearby_interactable`.
/// Read by the proximity prompt UI and the E-key handler.
#[derive(Resource, Default, Debug)]
pub struct NearbyInteractable {
    pub entity: Option<Entity>,
    pub kind: Option<InteractableKind>,
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
    // Initial transform is derived from CameraRig defaults so it matches
    // the first frame output of camera_follow_player.
    let default_rig = CameraRig::default();
    let pitch = default_rig.pitch_deg.to_radians();
    let arm = CAM_BASE_ARM * default_rig.zoom;
    let initial_cam_offset = Vec3::new(0.0, arm * pitch.sin(), arm * pitch.cos());
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(initial_cam_offset).looking_at(Vec3::ZERO, Vec3::Y),
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
        LogicalPos2d(Vec2::ZERO), // starts at tile (0,0) = world (0,0)
        IsWalking::default(),
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
    // Demo bank booths — wooden counter with a raised back panel
    // ------------------------------------------------------------------
    let counter_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.32, 0.10), // warm oak brown
        perceptual_roughness: 0.8,
        ..default()
    });
    let panel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.40, 0.24, 0.08), // darker backing panel
        perceptual_roughness: 0.85,
        ..default()
    });
    for (x, y) in [(-4i32, 1i32), (-5, 1)] {
        let booth_tile = TilePos::new(x, y);
        let base = tile_to_world3d(booth_tile);

        // Counter top — wide, low surface
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(TILE_SIZE * 0.92, TILE_SIZE * 0.45, TILE_SIZE * 0.80))),
            MeshMaterial3d(counter_mat.clone()),
            Transform::from_translation(base + Vec3::new(0.0, TILE_SIZE * 0.225, 0.0)),
            GridPos(booth_tile),
            BlocksMovement,
            InteractableKind::BankBooth,
        ));

        // Raised back panel
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(TILE_SIZE * 0.92, TILE_SIZE * 0.75, TILE_SIZE * 0.12))),
            MeshMaterial3d(panel_mat.clone()),
            Transform::from_translation(base + Vec3::new(0.0, TILE_SIZE * 0.375, TILE_SIZE * 0.34)),
        ));
    }

    // Target marker removed — no longer needed with WASD movement.
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
    rig: Res<CameraRig>,
    player_q: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut cam_q: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
) {
    let Ok(player_xform) = player_q.single() else { return; };
    let Ok(mut cam_xform) = cam_q.single_mut() else { return; };

    let pitch = rig.pitch_deg.to_radians();
    let arm = CAM_BASE_ARM * rig.zoom;
    let offset = Vec3::new(0.0, arm * pitch.sin(), arm * pitch.cos());

    // Snap directly — no lerp lag. Lag causes the player to drift in viewport
    // space each frame which makes smooth animations appear to jitter.
    cam_xform.translation = player_xform.translation + offset;
    cam_xform.look_at(player_xform.translation, Vec3::Y);
}

/// Scroll wheel → zoom. Arrow Up / Arrow Down (held) → pitch tilt.
pub fn camera_zoom_and_pitch(
    scroll: Res<AccumulatedMouseScroll>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
) {
    // Scroll wheel → zoom. Up = zoom in (shorter arm).
    let scroll_delta = scroll.delta.y;
    if scroll_delta != 0.0 {
        rig.zoom = (rig.zoom - scroll_delta * 0.15).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    // Arrow Up / Down → pitch tilt (hold to pan continuously).
    // 30°/sec means full range (25°→70°) takes ~1.5s — feels responsive but not jerky.
    let pitch_input =
        if keyboard.pressed(KeyCode::ArrowUp)   {  1.0_f32 }
        else if keyboard.pressed(KeyCode::ArrowDown) { -1.0_f32 }
        else { 0.0 };

    if pitch_input != 0.0 {
        rig.pitch_deg = (rig.pitch_deg + pitch_input * 30.0 * time.delta_secs())
            .clamp(PITCH_MIN_DEG, PITCH_MAX_DEG);
    }
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

// ---------------------------------------------------------------------------
// Root-motion correction (PostUpdate)
// ---------------------------------------------------------------------------

/// Re-apply the movement system's intended XZ position after Bevy's animation
/// system has had a chance to overwrite it with root-motion data from walk/idle
/// clips. Must run in PostUpdate, after animation, before transform propagation.
///
/// Two things are corrected each frame:
/// 1. The Player entity's world XZ is forced to `LogicalPos2d` — this handles
///    root motion applied directly to the SceneRoot entity.
/// 2. The XZ of every **direct child** of the Player entity (i.e. the GLB
///    Armature node) is zeroed out in local space — this handles root motion
///    applied to the Armature child, which would otherwise visually slide the
///    mesh past the destination tile before snapping on idle.
pub fn apply_logical_pos(
    mut player_q: Query<(&LogicalPos2d, &mut Transform, &Children), With<Player>>,
    mut child_xforms: Query<&mut Transform, Without<Player>>,
) {
    let Ok((logical, mut xform, children)) = player_q.single_mut() else { return; };

    // 1. Keep Player entity at logical movement position.
    xform.translation.x = logical.0.x;
    xform.translation.z = logical.0.y; // Vec2.y stores the world-Z axis

    // 2. Zero out any root-motion XZ translation on the direct GLB child node
    //    (usually the Armature). We preserve Y so the model doesn't sink into
    //    the ground if the animation bobs the root in Y.
    for child in children.iter() {
        if let Ok(mut child_xform) = child_xforms.get_mut(child) {
            child_xform.translation.x = 0.0;
            child_xform.translation.z = 0.0;
        }
    }
}
