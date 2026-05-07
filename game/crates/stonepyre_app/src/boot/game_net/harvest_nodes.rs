use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use stonepyre_engine::plugins::world::{
    player_feet_world, GridPos, InteractableKind, Player,
};
use stonepyre_world::tile_to_world_center;

use super::remote_players::RemoteNetPlayer;
use super::status::GameNetStatus;

const TREE_AVAILABLE_COLOR: Color = Color::srgb(0.2, 0.8, 0.2);
const TREE_DEPLETED_COLOR: Color = Color::srgb(0.34, 0.31, 0.24);

const DEFAULT_HARVEST_NODE_SPRITE_SCALE: f32 = 0.18;
const WORLD_OBJECT_DEPTH_BASE: f32 = 50.0;
const WORLD_OBJECT_DEPTH_STEP: f32 = 0.01;
const PLAYER_DEPTH_BIAS: f32 = 0.10;

#[derive(Default)]
pub(crate) struct HarvestNodeVisualCache {
    handles: HashMap<String, Handle<Image>>,
    manifests: HashMap<String, Option<HarvestNodeVisualManifest>>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HarvestNodeVisualManifest {
    pub visuals: HarvestNodeVisualSlots,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HarvestNodeVisualSlots {
    pub available: HarvestNodeVisualSlot,
    pub depleted: HarvestNodeVisualSlot,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct HarvestNodeVisualSlot {
    pub anchor: HarvestNodeVisualAnchor,
    #[serde(default = "default_visual_scale")]
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct HarvestNodeVisualAnchor {
    pub x: f32,
    pub y: f32,
}

fn default_visual_scale() -> f32 {
    1.0
}

/// Presentation-only bridge for server-owned harvest node state.
///
/// The server remains authoritative for whether a node can be harvested. This
/// system mirrors the latest `WorldSnapshot.harvest_nodes` state onto local
/// demo-world entities by using the sprite paths sent by the server.
///
/// To avoid a visible blank frame on available/depleted swaps, both sprite
/// handles are requested as soon as a node snapshot is seen, and the current
/// sprite is kept until the target image is loaded.
pub fn sync_harvest_node_visuals_from_server(
    status: Res<GameNetStatus>,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut cache: Local<HarvestNodeVisualCache>,
    mut tree_q: Query<(&GridPos, &mut Sprite, &mut Transform), With<InteractableKind>>,
) {
    if status.harvest_nodes.is_empty() {
        return;
    }

    for (grid_pos, mut sprite, mut transform) in &mut tree_q {
        let Some(node) = status.harvest_nodes.iter().find(|node| node.tile == grid_pos.0) else {
            continue;
        };

        let available_handle = ensure_sprite_handle(
            &mut cache,
            &asset_server,
            node.available_sprite.as_str(),
        );
        let depleted_handle = ensure_sprite_handle(
            &mut cache,
            &asset_server,
            node.depleted_sprite.as_str(),
        );

        let (target_path, target_handle) = if node.depleted {
            (node.depleted_sprite.as_str(), depleted_handle)
        } else {
            (node.available_sprite.as_str(), available_handle)
        };

        let Some(image) = images.get(&target_handle) else {
            // Keep the current visual alive while the requested sprite finishes
            // loading. This prevents a one-frame blank when the node depletes.
            if sprite.image == Handle::<Image>::default() {
                sprite.color = if node.depleted {
                    TREE_DEPLETED_COLOR
                } else {
                    TREE_AVAILABLE_COLOR
                };
            }
            continue;
        };

        let manifest = load_manifest_for_sprite_path(&mut cache, target_path);
        let slot = manifest
            .as_ref()
            .map(|m| if node.depleted { &m.visuals.depleted } else { &m.visuals.available });

        sprite.image = target_handle;
        sprite.color = Color::WHITE;
        sprite.custom_size = None;

        let tile_world = tile_to_world_center(node.tile);
        let image_size = Vec2::new(
            image.texture_descriptor.size.width as f32,
            image.texture_descriptor.size.height as f32,
        );

        let scale = slot
            .map(|s| s.scale * DEFAULT_HARVEST_NODE_SPRITE_SCALE)
            .unwrap_or(DEFAULT_HARVEST_NODE_SPRITE_SCALE);

        let anchor = slot
            .map(|s| s.anchor)
            .unwrap_or(HarvestNodeVisualAnchor { x: 0.5, y: 0.88 });

        transform.scale = Vec3::splat(scale);
        transform.translation.x = tile_world.x + (0.5 - anchor.x) * image_size.x * scale;
        transform.translation.y = tile_world.y + (anchor.y - 0.5) * image_size.y * scale;
        transform.translation.z = depth_for_world_y(tile_world.y);
    }
}

/// First-pass world layering.
///
/// South/lower entities should draw in front of north/higher entities. Players
/// use their feet position as the sorting anchor so large sprites can overlap
/// naturally as they walk in front of or behind trees.
pub fn update_world_object_depths(
    mut players: ParamSet<(
        Query<&mut Transform, With<Player>>,
        Query<&mut Transform, With<RemoteNetPlayer>>,
    )>,
) {
    for mut transform in &mut players.p0() {
        let feet_y = player_feet_world(&transform).y;
        transform.translation.z = depth_for_world_y(feet_y) + PLAYER_DEPTH_BIAS;
    }

    for mut transform in &mut players.p1() {
        let feet_y = player_feet_world(&transform).y;
        transform.translation.z = depth_for_world_y(feet_y) + PLAYER_DEPTH_BIAS;
    }
}

fn ensure_sprite_handle(
    cache: &mut HarvestNodeVisualCache,
    asset_server: &AssetServer,
    path: &str,
) -> Handle<Image> {
    if let Some(handle) = cache.handles.get(path) {
        return handle.clone();
    }

    let handle: Handle<Image> = asset_server.load(path.to_string());
    cache.handles.insert(path.to_string(), handle.clone());
    handle
}

fn load_manifest_for_sprite_path(
    cache: &mut HarvestNodeVisualCache,
    sprite_path: &str,
) -> Option<HarvestNodeVisualManifest> {
    let manifest_path = manifest_path_for_sprite_path(sprite_path)?;
    let key = manifest_path.to_string_lossy().to_string();

    if let Some(cached) = cache.manifests.get(&key) {
        return cached.clone();
    }

    let loaded = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<HarvestNodeVisualManifest>(&raw).ok());

    cache.manifests.insert(key, loaded.clone());
    loaded
}

fn manifest_path_for_sprite_path(sprite_path: &str) -> Option<PathBuf> {
    let rel = Path::new(sprite_path);
    let parent = rel.parent()?;
    Some(asset_root().join(parent).join("manifest.json"))
}

fn asset_root() -> PathBuf {
    std::env::var("STONEPYRE_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
}

fn depth_for_world_y(world_y: f32) -> f32 {
    WORLD_OBJECT_DEPTH_BASE - world_y * WORLD_OBJECT_DEPTH_STEP
}
