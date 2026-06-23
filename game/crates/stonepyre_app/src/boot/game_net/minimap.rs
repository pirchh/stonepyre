//! Placeholder minimap (top-right corner).
//!
//! A simple top-down radar centered on the local player, plotting live world
//! data each frame:
//! - obstacles (blocked tiles)        — muted brown
//! - harvest nodes (trees)            — green
//! - other players                    — cyan
//! - you, with a facing tick          — yellow
//!
//! Intentionally minimal (colored dots, no terrain art) but fully wired to the
//! real `WorldGrid` + network snapshots, so it tracks the world as you move.
//! When proper map art lands this can be swapped for a rendered map texture
//! without changing the data plumbing.

use std::collections::HashSet;

use bevy::prelude::*;

use stonepyre_engine::plugins::world::{player_feet_world, Facing, Player};
use stonepyre_world::{TilePos, TILE_SIZE};

use super::status::GameNetStatus;

const MINIMAP_SIZE: f32 = 160.0;
const MINIMAP_MARGIN: f32 = 12.0;
/// World-tiles visible from center to edge.
const VIEW_RADIUS_TILES: f32 = 16.0;
const DOT: f32 = 4.0;
const MAX_OBSTACLE_DOTS: usize = 400;

/// Top of the feedback-drop stack — sits just below the minimap.
pub const FEEDBACK_DROP_TOP: f32 = MINIMAP_MARGIN + MINIMAP_SIZE + 12.0;

#[derive(Component)]
pub struct MinimapRoot;

#[derive(Component)]
pub struct MinimapMarker;

/// Spawn the minimap panel on entering the world.
pub fn spawn_minimap(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(MINIMAP_MARGIN),
            top: Val::Px(MINIMAP_MARGIN),
            width: Val::Px(MINIMAP_SIZE),
            height: Val::Px(MINIMAP_SIZE),
            overflow: Overflow::clip(),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.08, 0.06, 0.85)),
        MinimapRoot,
        Name::new("minimap_panel"),
    ));
}

pub fn despawn_minimap(mut commands: Commands, panel_q: Query<Entity, With<MinimapRoot>>) {
    for entity in panel_q.iter() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.despawn();
        }
    }
}

/// Re-plot the minimap markers each frame, centered on the player.
pub fn update_minimap(
    mut commands: Commands,
    status: Res<GameNetStatus>,
    world: Res<stonepyre_world::WorldGrid>,
    panel_q: Query<Entity, With<MinimapRoot>>,
    marker_q: Query<Entity, With<MinimapMarker>>,
    player_q: Query<(&Transform, &Facing), With<Player>>,
) {
    let Ok(panel) = panel_q.single() else { return };
    // Clear last frame's markers, then re-plot fresh.
    for entity in marker_q.iter() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.despawn();
        }
    }

    let Ok((xform, facing)) = player_q.single() else { return };

    // Fractional tile position in the same space the rest of the game uses
    // (world3d_to_tile, which also feeds status.local_tile): feet = (world x, z),
    // tile.x = x / TILE_SIZE, tile.y = -z / TILE_SIZE. The Z negation is what keeps
    // north (higher tile.y) pointing up, matching the 3-D view.
    let feet = player_feet_world(xform);
    let player_x = feet.x / TILE_SIZE;
    let player_y = -feet.y / TILE_SIZE;

    let center = MINIMAP_SIZE * 0.5;
    let px_per_tile = (MINIMAP_SIZE * 0.5) / VIEW_RADIUS_TILES;

    // Tile center -> panel-local (left, top), or None if outside the view.
    let to_local = |tx: f32, ty: f32, size: f32| -> Option<(f32, f32)> {
        let dx = tx - player_x;
        let dy = ty - player_y;
        if dx.abs() > VIEW_RADIUS_TILES || dy.abs() > VIEW_RADIUS_TILES {
            return None;
        }
        // North (tile +y) is up, so screen-top decreases with +dy.
        let left = center + dx * px_per_tile - size * 0.5;
        let top = center - dy * px_per_tile - size * 0.5;
        Some((left, top))
    };

    // Harvest-node tiles (drawn as trees; also skipped in the obstacle pass).
    let tree_tiles: HashSet<TilePos> = status
        .harvest_nodes
        .iter()
        .filter(|n| !n.depleted)
        .map(|n| n.tile)
        .collect();

    // Obstacles — blocked tiles that aren't harvest nodes.
    let mut obstacles = 0usize;
    for tile in world.blocked.iter() {
        if tree_tiles.contains(tile) {
            continue;
        }
        if let Some((left, top)) = to_local(tile.x as f32, tile.y as f32, DOT) {
            spawn_marker(&mut commands, panel, left, top, DOT, Color::srgb(0.46, 0.38, 0.30));
            obstacles += 1;
            if obstacles >= MAX_OBSTACLE_DOTS {
                break;
            }
        }
    }

    // Trees.
    for tile in &tree_tiles {
        if let Some((left, top)) = to_local(tile.x as f32, tile.y as f32, DOT + 1.0) {
            spawn_marker(&mut commands, panel, left, top, DOT + 1.0, Color::srgb(0.32, 0.72, 0.36));
        }
    }

    // Other players.
    for p in &status.latest_players {
        if Some(p.player_id) == status.player_id {
            continue;
        }
        if let Some((left, top)) = to_local(p.tile.x as f32, p.tile.y as f32, DOT + 1.0) {
            spawn_marker(&mut commands, panel, left, top, DOT + 1.0, Color::srgb(0.34, 0.72, 0.92));
        }
    }

    // The player — a dot at center plus a small facing tick.
    spawn_marker(&mut commands, panel, center - 3.0, center - 3.0, 6.0, Color::srgb(0.96, 0.86, 0.32));
    let (fx, fy) = facing_tick_offset(*facing);
    spawn_marker(&mut commands, panel, center + fx - 1.5, center + fy - 1.5, 3.0, Color::WHITE);
}

fn spawn_marker(commands: &mut Commands, panel: Entity, left: f32, top: f32, size: f32, color: Color) {
    let marker = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(size),
                height: Val::Px(size),
                border_radius: BorderRadius::all(Val::Px(size * 0.5)),
                ..default()
            },
            BackgroundColor(color),
            MinimapMarker,
        ))
        .id();
    commands.entity(panel).add_child(marker);
}

/// Screen-space offset (px) of the facing tick relative to the player dot.
fn facing_tick_offset(facing: Facing) -> (f32, f32) {
    let d = 7.0;
    let diag = d * 0.7;
    match facing {
        Facing::North => (0.0, -d),
        Facing::South => (0.0, d),
        Facing::East => (d, 0.0),
        Facing::West => (-d, 0.0),
        Facing::NorthEast => (diag, -diag),
        Facing::NorthWest => (-diag, -diag),
        Facing::SouthEast => (diag, diag),
        Facing::SouthWest => (-diag, diag),
    }
}
