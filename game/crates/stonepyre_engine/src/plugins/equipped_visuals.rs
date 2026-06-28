//! Attaches the equipped main-hand item's 3-D model (currently axes) to the
//! player's right-hand bone, so it follows every animation automatically.
//!
//! This rig is awkward: its skeleton bone entities live in a different space
//! than the rendered (skinned) mesh — the hand bone entity sits ~48 units up
//! with a ~0.5 scale, while the visible hand is at ground level, because the
//! skinning shader applies each joint's inverse-bind matrix to the vertices.
//! So parenting a prop straight to the bone puts it in skeleton-space, far from
//! the visible hand.
//!
//! To land on the VISIBLE hand we fold the hand joint's inverse-bind matrix into
//! the model's local transform: with the model parented to the bone,
//!   model_world = bone_global * (inverse_bind * grip) = skinning_matrix * grip,
//! which is exactly where the skinning shader draws hand-bound vertices. The
//! grip is then authored in the mesh's own (≈ world-scale) space.

use bevy::prelude::*;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};

use stonepyre_content::items::ItemId;

use crate::plugins::inventory::Equipment;
use crate::plugins::world::Player;

// ---------------------------------------------------------------------------
// Grip — how a held model sits in the hand, authored in the mesh's bind space
// (≈ world units for this rig). Keyed by the item's tool kind, so each tool
// type (axe, future pickaxe / fishing rod / …) carries its own pose. Tiers
// within a kind share one grip since the GLBs are normalised identically.
// ---------------------------------------------------------------------------

/// A grip pose in the mesh's bind space.
struct Grip {
    translation: Vec3,
    /// Degrees, XYZ euler.
    rotation_deg: Vec3,
    scale: f32,
}

impl Grip {
    fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_euler(
                EulerRot::XYZ,
                self.rotation_deg.x.to_radians(),
                self.rotation_deg.y.to_radians(),
                self.rotation_deg.z.to_radians(),
            ),
            self.translation,
        )
    }
}

/// Axe grip — calibrated live on the flint axe; shared by every axe tier.
const AXE_GRIP: Grip = Grip {
    translation: Vec3::new(-0.85, 1.175, -0.3),
    rotation_deg: Vec3::new(60.0, 0.0, -11.25),
    scale: 1.025,
};

/// Resolve the grip matrix for a held item from its tool kind. When a new tool
/// type arrives, calibrate it once with temporary grip keys, then add a
/// `*_GRIP` const + a match arm here.
fn grip_for(item_id: &str) -> Mat4 {
    let kind = stonepyre_content::all_item_defs()
        .get(item_id)
        .and_then(|d| d.tool.as_ref())
        .map(|t| t.kind.as_str());
    match kind {
        Some("axe") => AXE_GRIP.matrix(),
        _ => AXE_GRIP.matrix(), // default until that kind is calibrated
    }
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Cached right-hand bone entity for a player, found once the rig spawns.
#[derive(Component)]
pub struct RightHandBoneLink(pub Entity);

/// The hand joint's inverse-bind matrix, resolved once from the skinned mesh.
#[derive(Component)]
pub struct HandBindMatrix(pub Mat4);

/// Tracks the currently-shown main-hand model so we only re-spawn on change.
#[derive(Component, Default)]
pub struct MainHandModel {
    /// Item id currently displayed (None = empty hand).
    pub item_id: Option<ItemId>,
    /// The spawned model entity parented under the hand bone.
    pub entity: Option<Entity>,
}

/// Marker on the spawned model root so `fit_main_hand_model` can pose it.
#[derive(Component)]
pub struct MainHandModelTag;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Walk a player's spawned glTF hierarchy to find the `mixamorig:RightHand`
/// bone and cache it as `RightHandBoneLink`. Retries each frame until ready.
pub fn link_right_hand_bone(
    mut commands: Commands,
    player_q: Query<Entity, (With<Player>, Without<RightHandBoneLink>)>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
) {
    for player in &player_q {
        if let Some(bone) = find_named(player, &children_q, &name_q) {
            info!("equipped_visuals: linked RightHand bone {bone:?} for player {player:?}");
            commands
                .entity(player)
                .insert((RightHandBoneLink(bone), MainHandModel::default()));
        }
    }
}

/// Resolve the hand joint's inverse-bind matrix from whichever `SkinnedMesh`
/// includes the hand bone. Runs until it succeeds, then inserts `HandBindMatrix`.
pub fn resolve_hand_bind_matrix(
    mut commands: Commands,
    player_q: Query<(Entity, &RightHandBoneLink), (With<Player>, Without<HandBindMatrix>)>,
    skinned_q: Query<&SkinnedMesh>,
    bindposes: Res<Assets<SkinnedMeshInverseBindposes>>,
) {
    for (player, hand) in &player_q {
        for skinned in &skinned_q {
            let Some(idx) = skinned.joints.iter().position(|&j| j == hand.0) else {
                continue;
            };
            let Some(ibp) = bindposes.get(&skinned.inverse_bindposes) else {
                continue;
            };
            let ib = ibp[idx];
            commands.entity(player).insert(HandBindMatrix(ib));
            info!("equipped_visuals: resolved hand inverse-bind matrix (joint idx {idx})");
            break;
        }
    }
}

/// Spawn / swap / despawn the main-hand model whenever `Equipment.main_hand`
/// changes for a player whose hand bone is linked.
pub fn sync_main_hand_model(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut player_q: Query<(&Equipment, &RightHandBoneLink, &mut MainHandModel), With<Player>>,
) {
    for (equipment, hand, mut model) in &mut player_q {
        let desired = equipment.main_hand.clone();
        if desired == model.item_id {
            continue; // no change
        }

        let path = desired.as_ref().and_then(|id| main_hand_model_path(id));
        info!(
            "equipped_visuals: main_hand {:?} -> {:?} (model={:?})",
            model.item_id, desired, path
        );

        // Drop the previous model (despawn is recursive in Bevy 0.18).
        if let Some(prev) = model.entity.take() {
            commands.entity(prev).despawn();
        }

        // Spawn under the hand bone. The local transform starts at zero scale
        // (invisible) until `fit_main_hand_model` poses it via the bind matrix.
        model.entity = path.map(|path| {
            let e = commands
                .spawn((
                    SceneRoot(asset_server.load(path)),
                    Transform::from_scale(Vec3::ZERO),
                    MainHandModelTag,
                    Name::new("MainHandModel"),
                ))
                .id();
            commands.entity(hand.0).add_child(e);
            info!("equipped_visuals: spawned main-hand model {e:?} under bone {:?}", hand.0);
            e
        });
        model.item_id = desired;
    }
}

/// Pose the model each frame: local transform = inverse_bind * grip, so its
/// world transform lands on the visible hand. Runs continuously to be robust to
/// the bind matrix / bone transform not being ready on the spawn frame.
pub fn fit_main_hand_model(
    player_q: Query<(&MainHandModel, &HandBindMatrix), With<Player>>,
    mut local_q: Query<&mut Transform, With<MainHandModelTag>>,
) {
    for (model, bind) in &player_q {
        let (Some(model_entity), Some(item_id)) = (model.entity, model.item_id.as_deref()) else {
            continue;
        };
        if let Ok(mut xf) = local_q.get_mut(model_entity) {
            *xf = Transform::from_matrix(bind.0 * grip_for(item_id));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Map a main-hand item id to its model asset path, if it has a 3-D model.
/// Currently only axes (tool kind "axe") have models, under `items/axes/`.
fn main_hand_model_path(item_id: &str) -> Option<String> {
    let def = stonepyre_content::all_item_defs().get(item_id)?;
    let tool = def.tool.as_ref()?;
    (tool.kind == "axe").then(|| format!("items/axes/{item_id}.glb#Scene0"))
}

/// True for the wrist/hand bone but not the finger bones. Matches any exporter
/// naming because finger bones carry a suffix (…RightHandIndex1).
fn is_right_hand(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with("righthand")
}

/// Recursive search of the hierarchy for the first right-hand bone by `Name`.
fn find_named(
    entity: Entity,
    children_q: &Query<&Children>,
    name_q: &Query<&Name>,
) -> Option<Entity> {
    if let Ok(name) = name_q.get(entity) {
        if is_right_hand(name.as_str()) {
            return Some(entity);
        }
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_named(child, children_q, name_q) {
                return Some(found);
            }
        }
    }
    None
}
