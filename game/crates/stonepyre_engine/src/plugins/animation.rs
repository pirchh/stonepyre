use bevy::prelude::*;
use bevy::animation::graph::AnimationNodeIndex;
use bevy::gltf::Gltf;
use std::time::Duration;

use crate::plugins::interaction::AutoChopActive;
use crate::plugins::movement::IsWalking;
use crate::plugins::skills::{AnimClip, RequestedAnim};
use crate::plugins::world::{Facing, LogicalPos2d, Player};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Which 3-D animation clip is currently playing on the player.
#[derive(Component)]
pub struct HumanoidAnim3d {
    pub current: Option<AnimClip3d>,
    pub last_facing: Option<Facing>,
    /// The Y-rotation we are slerping toward this frame.
    /// Updated whenever `Facing` changes; the mesh smoothly turns each frame.
    pub target_rotation: Quat,
}

impl Default for HumanoidAnim3d {
    fn default() -> Self {
        Self {
            current: None,
            last_facing: None,
            target_rotation: Quat::IDENTITY, // faces South (model default)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimClip3d {
    Idle,
    Walk,
    Woodcutting,
}

/// Inserted alongside `RequestedAnim` on the first frame the clip starts playing.
/// Prevents `consume_requested_anim` from restarting the clip every frame.
#[derive(Component)]
pub struct RequestedAnimStarted;

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
    pub woodcutting: Option<AnimationNodeIndex>,
    /// Raw handle so callers can look up the woodcutting clip duration.
    pub woodcutting_clip: Option<Handle<AnimationClip>>,
    /// Duration of the walk clip in seconds — used to maintain a continuous
    /// walk phase so re-entering walk state doesn't restart from frame 0.
    pub walk_duration: f32,
    /// Raw handle kept so `animate_humanoid` can look up the duration once loaded.
    pub walk_clip: Option<Handle<AnimationClip>>,
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

    let walk_clip = gltf.named_animations.get("walk").cloned();
    let walk_node = walk_clip
        .as_ref()
        .map(|h| graph.add_clip(h.clone(), 1.0, graph.root));

    let woodcutting_clip = gltf.named_animations.get("woodcutting").cloned();
    let woodcutting_node = woodcutting_clip
        .as_ref()
        .map(|h| graph.add_clip(h.clone(), 1.0, graph.root));

    info!(
        "player.glb animations found: {:?} | idle={} walk={} woodcutting={}",
        gltf.named_animations.keys().collect::<Vec<_>>(),
        idle_node.is_some(), walk_node.is_some(), woodcutting_node.is_some(),
    );
    if idle_node.is_none() {
        warn!("player.glb: no animation named 'idle' found");
    }
    if walk_node.is_none() {
        warn!("player.glb: no animation named 'walk' found");
    }
    if woodcutting_node.is_none() {
        warn!("player.glb: no animation named 'woodcutting' found — axe swing will be skipped");
    }

    anim_graph_res.graph = animation_graphs.add(graph);
    anim_graph_res.idle = idle_node;
    anim_graph_res.walk = walk_node;
    anim_graph_res.woodcutting = woodcutting_node;
    anim_graph_res.woodcutting_clip = woodcutting_clip;
    anim_graph_res.walk_clip = walk_clip;
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

        // Attach the graph and transitions component to the AnimationPlayer entity.
        commands
            .entity(anim_entity)
            .insert(AnimationGraphHandle(anim_graph_res.graph.clone()))
            .insert(AnimationTransitions::new());

        // Start idle immediately (no transition needed on first play).
        if let Ok(mut player) = anim_player_mut.get_mut(anim_entity) {
            player.play(idle_node).repeat();
        }
        // AnimationTransitions is already inserted above; nothing else needed here.

        commands
            .entity(player_entity)
            .insert(AnimPlayerLink(anim_entity));
    }
}

/// Drive the `AnimationPlayer` each frame based on whether the player is
/// walking or idle, and rotate the root transform to match `Facing`.
pub fn animate_humanoid(
    time: Res<Time>,
    mut log_timer: Local<f32>,
    // walk_elapsed: continuous clock, never resets — used to seek to the
    // correct phase when re-entering walk state so the cycle doesn't restart
    // from frame 0 on every tile click.
    mut walk_elapsed: Local<f32>,
    // walk_duration_cache: resolved once from Assets<AnimationClip>.
    mut walk_duration_cache: Local<f32>,
    anim_clips: Res<Assets<AnimationClip>>,
    anim_graph_res: Res<PlayerAnimGraph>,
    mut player_q: Query<
        (
            &Facing,
            &IsWalking,
            &AnimPlayerLink,
            &mut HumanoidAnim3d,
            &mut Transform,
            Option<&ForceIdleOnce>,
            &LogicalPos2d,
            Option<&RequestedAnim>,
        ),
        With<Player>,
    >,
    mut anim_player_q: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut commands: Commands,
    player_entity_q: Query<Entity, With<Player>>,
) {
    let (Some(idle_node), Some(walk_node)) = (anim_graph_res.idle, anim_graph_res.walk) else {
        return;
    };

    let Ok((facing, is_walking, anim_link, mut anim, mut xform, force_idle, logical, req_anim)) =
        player_q.single_mut()
    else {
        return;
    };

    let Ok((mut ap, mut transitions)) = anim_player_q.get_mut(anim_link.0) else {
        return;
    };

    // Advance the continuous walk phase clock every frame regardless of
    // whether the player is currently walking or idle.
    *walk_elapsed += time.delta_secs();

    // -------------------------------------------------------------------
    // Force idle snap (used when e.g. stopping a woodcutting action)
    // -------------------------------------------------------------------
    if force_idle.is_some() {
        if let Ok(ent) = player_entity_q.single() {
            commands.entity(ent).remove::<ForceIdleOnce>();
        }
        transitions.play(&mut ap, idle_node, Duration::ZERO).repeat();
        anim.current = Some(AnimClip3d::Idle);
        anim.last_facing = Some(*facing);
    }

    // -------------------------------------------------------------------
    // Choose clip — driven directly by WASD input via IsWalking
    // Skip if consume_requested_anim owns the animation player right now.
    // -------------------------------------------------------------------
    if req_anim.is_some() {
        if is_walking.0 {
            // Player moved away — immediately cancel woodcutting and auto-chop.
            if let Ok(ent) = player_entity_q.single() {
                commands.entity(ent).remove::<RequestedAnim>();
                commands.entity(ent).remove::<RequestedAnimStarted>();
                commands.entity(ent).remove::<AutoChopActive>();
            }
            // Fall through so the walk animation starts this frame.
        } else {
            // Still chopping — update facing toward the target and hold.
            if anim.last_facing != Some(*facing) {
                use std::f32::consts::PI;
                let angle = match *facing {
                    Facing::South     =>  0.0,
                    Facing::SouthEast =>  PI / 4.0,
                    Facing::East      =>  PI / 2.0,
                    Facing::NorthEast =>  3.0 * PI / 4.0,
                    Facing::North     =>  PI,
                    Facing::NorthWest => -3.0 * PI / 4.0,
                    Facing::West      => -PI / 2.0,
                    Facing::SouthWest => -PI / 4.0,
                };
                anim.target_rotation = Quat::from_rotation_y(angle);
                anim.last_facing = Some(*facing);
            }
            let t = (time.delta_secs() * 15.0).min(1.0);
            xform.rotation = xform.rotation.slerp(anim.target_rotation, t);
            return;
        }
    }

    let target_clip = if is_walking.0 {
        AnimClip3d::Walk
    } else {
        AnimClip3d::Idle
    };

    // Resolve walk clip duration once (asset may not be ready on first frame).
    if *walk_duration_cache == 0.0 {
        if let Some(dur) = anim_graph_res
            .walk_clip
            .as_ref()
            .and_then(|h| anim_clips.get(h))
            .map(|c| c.duration())
        {
            *walk_duration_cache = dur;
        }
    }

    if anim.current != Some(target_clip) {
        let node = if target_clip == AnimClip3d::Walk {
            walk_node
        } else {
            idle_node
        };
        // Both transitions instant — OSRS-style tile movement cuts directly
        // between walk and idle without blending.
        let active = transitions.play(&mut ap, node, Duration::ZERO);
        // When re-entering walk state, seek to the current phase of the
        // continuous walk clock so the cycle resumes where it would have been
        // rather than snapping back to frame 0 on every tile click.
        if target_clip == AnimClip3d::Walk && *walk_duration_cache > 0.0 {
            let phase = *walk_elapsed % *walk_duration_cache;
            active.seek_to(phase);
        }
        active.repeat();
        anim.current = Some(target_clip);
    }

    // -------------------------------------------------------------------
    // Facing → Y-axis rotation
    // Mixamo/Blender GLB exports the character facing +Z (toward camera).
    //   South (tile Y-) = world +Z = 0°   (model default, faces camera)
    //   North (tile Y+) = world -Z = 180°
    //   East  (tile X+) = world +X = +90° (turn left from front)
    //   West  (tile X-) = world -X = -90° (turn right from front)
    // -------------------------------------------------------------------
    // When facing changes, update the target rotation.
    // The mesh slerps toward it every frame rather than snapping.
    if anim.last_facing != Some(*facing) {
        use std::f32::consts::PI;
        // Rotation around Y: positive = CCW from above.
        // Model default faces +Z (South = toward camera) at angle 0.
        let angle = match *facing {
            Facing::South     =>  0.0,
            Facing::SouthEast =>  PI / 4.0,
            Facing::East      =>  PI / 2.0,
            Facing::NorthEast =>  3.0 * PI / 4.0,
            Facing::North     =>  PI,
            Facing::NorthWest => -3.0 * PI / 4.0,
            Facing::West      => -PI / 2.0,
            Facing::SouthWest => -PI / 4.0,
        };
        anim.target_rotation = Quat::from_rotation_y(angle);
        anim.last_facing = Some(*facing);
    }

    // Smoothly rotate toward the target facing every frame.
    // t = 15 × dt gives a ~3-frame turn at 60 fps — snappy but not instant.
    // Quat::slerp always takes the short arc, so ±PI wrapping is handled.
    let t = (time.delta_secs() * 15.0).min(1.0);
    xform.rotation = xform.rotation.slerp(anim.target_rotation, t);
}

/// Drives `RequestedAnim` one-shots (e.g. woodcutting swing).
///
/// On the first frame: starts the clip (no repeat — plays once).
/// Every frame: ticks the timer.
/// When the timer expires: transitions back to idle and removes `RequestedAnim`.
///
/// Must run BEFORE `animate_humanoid` so that when the one-shot finishes,
/// `animate_humanoid` immediately resumes idle/walk in the same frame.
pub fn consume_requested_anim(
    time:           Res<Time>,
    anim_graph_res: Res<PlayerAnimGraph>,
    mut player_q:   Query<
        (Entity, &mut RequestedAnim, &AnimPlayerLink, &mut HumanoidAnim3d, Has<RequestedAnimStarted>),
        With<Player>,
    >,
    mut anim_player_q: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut commands:   Commands,
) {
    let Ok((player_ent, mut req, anim_link, mut anim, already_started)) = player_q.single_mut()
    else {
        return;
    };
    let Ok((mut ap, mut transitions)) = anim_player_q.get_mut(anim_link.0) else { return };

    // First frame: start the clip.
    if !already_started {
        let node = match req.clip {
            AnimClip::Woodcutting => anim_graph_res.woodcutting,
        };

        let mode_name = if req.mode.is_one_shot() { "OneShot" } else { "Loop" };
        info!(
            "consume_requested_anim: starting clip={:?} node_found={} mode={}",
            req.clip, node.is_some(), mode_name,
        );

        if let Some(n) = node {
            let active = transitions.play(&mut ap, n, Duration::ZERO);
            match &req.mode {
                crate::plugins::skills::RequestedAnimMode::Loop { .. } => { active.repeat(); }
                crate::plugins::skills::RequestedAnimMode::OneShot { .. } => { /* plays once */ }
            }
            anim.current = Some(AnimClip3d::Woodcutting);
        }
        commands.entity(player_ent).insert(RequestedAnimStarted);
    }

    // Loop mode: Bevy's native .repeat() handles looping.
    // RequestedAnim is removed externally when the action ends — no timer check needed.
    if req.mode.is_one_shot() {
        req.mode.tick(time.delta());

        if req.mode.just_finished() {
            info!("consume_requested_anim: OneShot timer fired, returning to idle");
            if let Some(idle) = anim_graph_res.idle {
                transitions.play(&mut ap, idle, Duration::ZERO).repeat();
                anim.current = Some(AnimClip3d::Idle);
            }
            commands.entity(player_ent).remove::<RequestedAnim>();
            commands.entity(player_ent).remove::<RequestedAnimStarted>();
        }
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
