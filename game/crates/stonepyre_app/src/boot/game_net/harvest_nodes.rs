use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use stonepyre_engine::plugins::world::{BlocksMovement, GridPos, InteractableKind};
use stonepyre_world::tile_to_world3d;

use super::protocol::HarvestNodeSnapshot;
use super::status::GameNetStatus;

/// Same scale constant used for the player GLB.
/// All assets from the pipeline are exported in metres (1 Blender unit = 1 m,
/// scale baked into vertices via transform_apply). Multiplying by 53.3 converts
/// metres → world units so everything lands at the correct size relative to the
/// 64wu tile grid.
///
///   Oak tree:  4.2 m × 53.3 = 223.9 wu ≈ 3.5 tiles tall  ✓
///   Player:    1.8 m × 53.3 =  95.9 wu ≈ 1.5 tiles tall  ✓
pub const HARVEST_NODE_SCALE: f32 = 53.3;

/// Tracks the spawned visual state for a server harvest node.
#[derive(Component, Clone, Debug)]
pub(crate) struct ServerHarvestNodeVisual {
    pub node_id: String,
    /// GLB path currently loaded for this entity so we know when to swap.
    pub current_scene_path: String,
}

/// Caches `Handle<Scene>` by GLB path so we don't re-load the same asset
/// on every snapshot tick.
#[derive(Default)]
pub(crate) struct HarvestNodeSceneCache {
    handles: HashMap<String, Handle<Scene>>,
}

/// Mirrors server harvest node state into 3-D scene entities.
///
/// - Nodes present in the snapshot but not yet spawned are spawned.
/// - Nodes whose depletion state has changed since last spawn are
///   despawned and re-spawned with the correct GLB (tree ↔ stump).
/// - Nodes no longer in the snapshot (e.g. server removed them) are
///   despawned.
pub fn sync_harvest_node_visuals_from_server(
    mut commands: Commands,
    status: Res<GameNetStatus>,
    asset_server: Res<AssetServer>,
    mut cache: Local<HarvestNodeSceneCache>,
    node_q: Query<(Entity, &ServerHarvestNodeVisual)>,
) {
    if !status.connected {
        return;
    }

    // Build a map of what is currently spawned: node_id → (entity, current_scene_path)
    let spawned: HashMap<&str, (Entity, &str)> = node_q
        .iter()
        .map(|(e, v)| (v.node_id.as_str(), (e, v.current_scene_path.as_str())))
        .collect();

    // Build the set of node_ids the server currently reports.
    let server_ids: HashSet<&str> = status
        .harvest_nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    // Despawn any entity no longer in the server snapshot.
    for (node_id, (entity, _)) in &spawned {
        if !server_ids.contains(node_id) {
            commands.entity(*entity).despawn();
        }
    }

    for node in &status.harvest_nodes {
        let wanted_path = scene_path_for_node(node);

        match spawned.get(node.node_id.as_str()) {
            // Already spawned with the right scene — nothing to do.
            Some((_, current)) if *current == wanted_path => {}

            // Spawned but wrong scene (depletion changed) — swap it out.
            Some((entity, _)) => {
                commands.entity(*entity).despawn();
                spawn_harvest_node_3d(&mut commands, &asset_server, &mut cache, node, wanted_path);
            }

            // Not yet spawned — create it.
            None => {
                spawn_harvest_node_3d(&mut commands, &asset_server, &mut cache, node, wanted_path);
            }
        }
    }
}

fn spawn_harvest_node_3d(
    commands: &mut Commands,
    asset_server: &AssetServer,
    cache: &mut HarvestNodeSceneCache,
    node: &HarvestNodeSnapshot,
    scene_path: &str,
) {
    let handle = cache
        .handles
        .entry(scene_path.to_string())
        .or_insert_with(|| asset_server.load(format!("{}#Scene0", scene_path)))
        .clone();

    let pos = tile_to_world3d(node.tile);

    commands.spawn((
        SceneRoot(handle),
        Transform::from_translation(pos).with_scale(Vec3::splat(HARVEST_NODE_SCALE)),
        GridPos(node.tile),
        BlocksMovement,
        InteractableKind::Tree,
        ServerHarvestNodeVisual {
            node_id: node.node_id.clone(),
            current_scene_path: scene_path.to_string(),
        },
    ));
}

/// Returns the GLB asset path that should be loaded for this node's current state.
/// `available_sprite` / `depleted_sprite` are repurposed as GLB paths —
/// the field names are a legacy of the old 2D pipeline.
fn scene_path_for_node(node: &HarvestNodeSnapshot) -> &str {
    if node.depleted {
        node.depleted_sprite.as_str()
    } else {
        node.available_sprite.as_str()
    }
}
