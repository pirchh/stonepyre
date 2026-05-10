use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::world::{GridPos, InteractableKind};
use stonepyre_world::{tile_to_world_center, TilePos, TILE_SIZE};

use super::protocol::GroundItemSnapshot;
use super::status::GameNetStatus;

const GROUND_ITEM_COLOR: Color = Color::srgb(0.88, 0.70, 0.30);
const GROUND_ITEM_SIZE: f32 = TILE_SIZE * 0.36;
const GROUND_ITEM_DEPTH: f32 = 80.0;
const GROUND_ITEM_TEXT_DEPTH: f32 = 81.0;

#[derive(Component, Clone, Debug)]
pub(crate) struct ServerGroundItemVisual {
    pub ground_item_id: Uuid,
}

#[derive(Component)]
pub(crate) struct ServerGroundItemLabel;

pub fn sync_ground_item_visuals_from_server(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    status: Res<GameNetStatus>,
    ground_item_q: Query<(Entity, &ServerGroundItemVisual), Without<ServerGroundItemLabel>>,
) {
    let dominant_ids = dominant_ground_item_ids_by_tile(&status.ground_items);
    let desired_ids: HashSet<Uuid> = status
        .ground_items
        .iter()
        .map(|item| item.ground_item_id)
        .collect();

    let mut existing: HashMap<Uuid, Entity> = HashMap::new();
    for (entity, visual) in ground_item_q.iter() {
        if desired_ids.contains(&visual.ground_item_id) {
            existing.insert(visual.ground_item_id, entity);
        } else if let Ok(mut ec) = commands.get_entity(entity) {
            ec.despawn();
        }
    }

    for item in &status.ground_items {
        let should_show_label = dominant_ids.get(&item.tile) == Some(&item.ground_item_id);

        if let Some(entity) = existing.get(&item.ground_item_id).copied() {
            update_existing_ground_item_label(
                &mut commands,
                entity,
                &asset_server,
                item,
                should_show_label,
            );
            continue;
        }

        spawn_ground_item_visual(&mut commands, &asset_server, item, should_show_label);
    }
}

fn dominant_ground_item_ids_by_tile(items: &[GroundItemSnapshot]) -> HashMap<TilePos, Uuid> {
    let mut dominant: HashMap<TilePos, &GroundItemSnapshot> = HashMap::new();

    for item in items {
        dominant
            .entry(item.tile)
            .and_modify(|current| {
                if is_more_dominant(item, current) {
                    *current = item;
                }
            })
            .or_insert(item);
    }

    dominant
        .into_iter()
        .map(|(tile, item)| (tile, item.ground_item_id))
        .collect()
}

fn is_more_dominant(candidate: &GroundItemSnapshot, current: &GroundItemSnapshot) -> bool {
    candidate
        .quantity
        .cmp(&current.quantity)
        .then_with(|| item_display_name(&candidate.item_id).cmp(&item_display_name(&current.item_id)))
        .then_with(|| candidate.ground_item_id.cmp(&current.ground_item_id))
        .is_gt()
}

fn update_existing_ground_item_label(
    commands: &mut Commands,
    root: Entity,
    asset_server: &AssetServer,
    item: &GroundItemSnapshot,
    should_show_label: bool,
) {
    if !should_show_label {
        return;
    }

    let label = ground_item_label(item);
    let font: Handle<Font> = asset_server.load("fonts/ui.ttf");
    let text = commands
        .spawn((
            Text2d::new(label),
            TextFont {
                font,
                font_size: 13.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.90, 0.68)),
            Transform::from_xyz(0.0, TILE_SIZE * 0.32, GROUND_ITEM_TEXT_DEPTH - GROUND_ITEM_DEPTH),
            ServerGroundItemLabel,
            Visibility::Visible,
            Name::new("ground_item_label"),
        ))
        .id();

    commands.entity(root).add_child(text);
}

fn spawn_ground_item_visual(
    commands: &mut Commands,
    asset_server: &AssetServer,
    item: &GroundItemSnapshot,
    should_show_label: bool,
) {
    let world = tile_to_world_center(item.tile);
    let display_name = item_display_name(&item.item_id);

    let root = commands
        .spawn((
            Sprite::from_color(GROUND_ITEM_COLOR, Vec2::splat(GROUND_ITEM_SIZE)),
            Transform::from_xyz(world.x, world.y, GROUND_ITEM_DEPTH),
            GridPos(item.tile),
            InteractableKind::GroundItem {
                display_name: display_name.clone(),
            },
            ServerGroundItemVisual {
                ground_item_id: item.ground_item_id,
            },
            Visibility::Visible,
            Name::new(format!("ground_item_{}", item.item_id)),
        ))
        .id();

    if should_show_label {
        update_existing_ground_item_label(commands, root, asset_server, item, true);
    }
}

fn ground_item_label(item: &GroundItemSnapshot) -> String {
    let display_name = item_display_name(&item.item_id);
    if item.quantity > 1 {
        format!("{} x{}", display_name, item.quantity)
    } else {
        display_name
    }
}

fn item_display_name(item_id: &str) -> String {
    default_item_defs()
        .get(item_id)
        .map(|def| def.name.clone())
        .unwrap_or_else(|| item_id.to_string())
}
