use bevy::prelude::*;
use uuid::Uuid;

use stonepyre_world::TilePos;

use super::status::GameNetStatus;

#[derive(Component)]
pub struct GameNetOverlayRoot;

#[derive(Component)]
pub struct GameNetOverlayText;

pub fn spawn_game_net_overlay(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/ui.ttf");

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(14.0),
                width: Val::Px(420.0),
                height: Val::Auto,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.025, 0.72)),
            GameNetOverlayRoot,
            Name::new("game_net_debug_overlay"),
        ))
        .id();

    let text = commands
        .spawn((
            Text::new("Game Net: starting..."),
            TextFont {
                font,
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.88, 0.92, 0.95)),
            GameNetOverlayText,
            Name::new("game_net_debug_overlay_text"),
        ))
        .id();

    commands.entity(root).add_child(text);
}

pub fn despawn_game_net_overlay(
    mut commands: Commands,
    roots: Query<Entity, With<GameNetOverlayRoot>>,
) {
    for e in roots.iter() {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
}

pub fn update_game_net_overlay(
    status: Res<GameNetStatus>,
    mut text_q: Query<&mut Text, With<GameNetOverlayText>>,
) {
    let Ok(mut text) = text_q.single_mut() else {
        return;
    };

    let connection = if status.connected {
        "Connected"
    } else if status.connecting {
        "Connecting"
    } else {
        "Disconnected"
    };

    text.0 = format!(
        "Game Net\n\
         Connection: {connection}\n\
         Player ID: {}\n\
         Character ID: {}\n\
         Tick Hz: {}\n\
         Snapshot Tick: {}\n\
         Snapshot Players: {}\n\
         Remote Players: {}\n\
         Local Tile: {}\n\
         Server Tile: {}\n\
         Drift: {}\n\
         Last Move Sent: {}\n\
         Corrections: {}\n\
         Last Error: {}",
        fmt_uuid(status.player_id),
        fmt_uuid(status.character_id),
        status.tick_hz.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
        status.server_tick.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
        status.snapshot_players,
        status.remote_player_count,
        fmt_tile(status.local_tile),
        fmt_tile(status.server_tile),
        status.drift_tiles.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
        fmt_tile(status.last_move_sent),
        status.correction_count,
        status.last_error.clone().unwrap_or_else(|| "—".to_string()),
    );
}

fn fmt_uuid(v: Option<Uuid>) -> String {
    v.map(|id| id.to_string()).unwrap_or_else(|| "—".to_string())
}

fn fmt_tile(v: Option<TilePos>) -> String {
    v.map(|t| format!("{}, {}", t.x, t.y))
        .unwrap_or_else(|| "—".to_string())
}
