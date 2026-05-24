use bevy::prelude::*;
use bevy::animation::graph::AnimationNodeIndex;
use bevy::gltf::Gltf;

use crate::plugins::movement::StepTo;
use crate::plugins::world::{Facing, Player, TilePath};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Which 3-D animation clip is currently playing on the player.
#[derive(Component, Default)]
pub struct HumanoidAnim3d {
    pub current: Option<AnimClip3d>,
    pub last_facing: Option<Facing>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimClip3d {
    Idle,
    Walk,
}

/// Cached reference to the child entity that owns `AnimationPlayer`.
/// Inserted once the scene hierarchy is fully spawned.
#[derive(Component)]
pub struct AnimPlayerLink(pub Entity);

/// Marker so the camera and other systems know which entity is the 3-D player root.
/// (Re-exported convenience — Player already exists, this is just an alias comment.)

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// Handle to the loaded player GLB file.
#[derive(Resource, Default)]
pub struct PlayerGltfHandle(pub Handle<Gltf>);

/// Resolved animation graph + node indices, built once the GLB is loaded.
#[derive(Resource, Default)]
pub struct PlayerAnimGraph {
    pub graph: Handle<AnimationGraph>,
    pub idle: Option<AnimationNodeIndex>,
    pub walk: Option<AnimationNodeIndex>,
}

// ---------------------------------------------------------------------------
// Keep legacy types so the rest of the engine compiles unchanged
// (RequestedAnim, AnimClip, ForceIdleOnce are still set by interaction code)
// ---------------------------------------------------------------------------

/// Insert this when you want to HARD snap the player back to idle *this frame*.
#[derive(Component)]
pub struct ForceIdleOnce;

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Once the GLB asset is loaded, build an `AnimationGraph` with idle + walk nodes.
pub fn setup_player_anim_graph(
    mut anim_graph_res: ResMut<PlayerAnimGraph>,
    gltf_handle: Res<PlayerGltfHandle>,
    gltf_assets: Res<Assets<Gltf>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    // Already set up or not loaded yet.
    if anim_graph_res.idle.is_some() {
        return;
    }
    let Some(gltf) = gltf_assets.get(&gltf_handle.0) else {
        return;
    };

    let mut graph = AnimationGraph::new();

    let idle_node = gltf
        .named_animations
        .get("idle")
        .map(|h| graph.add_clip(h.clone(), 1.0, graph.root));

    let walk_node = gltf
        .named_animations
        .get("walk")
        .map(|h| graph.add_clip(h.clone(), 1.0, graph.root));

    if idle_node.is_none() {
        warn!("player.glb: no animation named 'idle' found");
    }
    if walk_node.is_none() {
        warn!("player.glb: no animation named 'walk' found");
    }

    anim_graph_res.graph = animation_graphs.add(graph);
    anim_graph_res.idle = idle_node;
    anim_graph_res.walk = walk_node;
}

/// Walk the scene hierarchy of player entities to find their `AnimationPlayer`
/// child, then wire up the `AnimationGraph` and insert `AnimPlayerLink`.
pub fn link_anim_player_to_player(
    mut commands: Commands,
    anim_graph_res: Res<PlayerAnimGraph>,
    player_q: Query<Entity, (With<Player>, Without<AnimPlayerLink>)>,
    children_q: Query<&Children>,
    anim_player_q: Query<Entity, With<AnimationPlayer>>,
    mut anim_player_mut: Query<&mut AnimationPlayer>,
) {
    // Don't run until the graph is ready.
    let Some(idle_node) = anim_graph_res.idle else {
        return;
    };

    for player_entity in &player_q {
        let Some(anim_entity) =
            find_in_children(player_entity, &children_q, &anim_player_q)
        else {
            continue; // scene not spawned yet, retry next frame
        };

        // Attach the graph to the AnimationPlayer entity.
        commands
            .entity(anim_entity)
            .insert(AnimationGraphHandle(anim_graph_res.graph.clone()));

        // Start idle immediately.
        if let Ok(mut player) = anim_player_mut.get_mut(anim_entity) {
            player.play(idle_node).repeat();
        }

        commands
            .entity(player_entity)
            .insert(AnimPlayerLink(anim_entity));
    }
}

/// Drive the `AnimationPlayer` each frame based on whether the player is
/// walking or idle, and rotate the root transform to match `Facing`.
pub fn animate_humanoid(
    anim_graph_res: Res<PlayerAnimGraph>,
    mut player_q: Query<
        (
            &Facing,
            &TilePath,
            Option<&StepTo>,
            &AnimPlayerLink,
            &mut HumanoidAnim3d,
            &mut Transform,
            Option<&ForceIdleOnce>,
        ),
        With<Player>,
    >,
    mut anim_player_q: Query<&mut AnimationPlayer>,
    mut commands: Commands,
    player_entity_q: Query<Entity, With<Player>>,
) {
    let (Some(idle_node), Some(walk_node)) = (anim_graph_res.idle, anim_graph_res.walk) else {
        return;
    };

    let Ok((facing, path, step_to, anim_link, mut anim, mut xform, force_idle)) =
        player_q.single_mut()
    else {
        return;
    };

    let Ok(mut ap) = anim_player_q.get_mut(anim_link.0) else {
        return;
    };

    // -------------------------------------------------------------------
    // Force idle snap (used when e.g. stopping a woodcutting action)
    // -------------------------------------------------------------------
    if force_idle.is_some() {
        if let Ok(ent) = player_entity_q.single() {
            commands.entity(ent).remove::<ForceIdleOnce>();
        }
        ap.play(idle_node).repeat();
        anim.current = Some(AnimClip3d::Idle);
        anim.last_facing = Some(*facing);
    }

    // -------------------------------------------------------------------
    // Choose clip
    // -------------------------------------------------------------------
    let walking = step_to.is_some() || !path.tiles.is_empty();
    let target_clip = if walking {
        AnimClip3d::Walk
    } else {
        AnimClip3d::Idle
    };

    if anim.current != Some(target_clip) {
        let node = if target_clip == AnimClip3d::Walk {
            walk_node
        } else {
            idle_node
        };
        ap.play(node).repeat();
        anim.current = Some(target_clip);
    }

    // -------------------------------------------------------------------
    // Facing → Y-axis rotation
    // Model exported from Blender faces -Z by default (glTF convention).
    //   North (tile Y+) = world -Z = 0° (model default)
    //   South (tile Y-) = world +Z = 180°
    //   East  (tile X+) = world +X = -90° (turn right)
    //   West  (tile X-) = world -X = +90° (turn left)
    // Adjust these constants if the model faces a different direction.
    // -------------------------------------------------------------------
    if anim.last_facing != Some(*facing) {
        use std::f32::consts::PI;
        let angle = match *facing {
            Facing::North => 0.0,
            Facing::South => PI,
            Facing::East => -PI / 2.0,
            Facing::West => PI / 2.0,
        };
        xform.rotation = Quat::from_rotation_y(angle);
        anim.last_facing = Some(*facing);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_in_children(
    entity: Entity,
    children_q: &Query<&Children>,
    target_q: &Query<Entity, With<AnimationPlayer>>,
) -> Option<Entity> {
    if target_q.get(entity).is_ok() {
        return Some(entity);
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_in_children(child, children_q, target_q) {
                return Some(found);
            }
        }
    }
    None
}
