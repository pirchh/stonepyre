use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::thread;
use tungstenite::{client::IntoClientRequest, connect, Message};
use uuid::Uuid;

use stonepyre_world::{world_to_tile, TilePos};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum ClientMsg {
    Ping,
    JoinWorld { character_id: Uuid },
    MoveTo { tile: TilePos },
}

#[derive(Debug, Clone, Copy)]
enum GameNetCommand {
    MoveTo { tile: TilePos },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum ServerMsg {
    Pong,
    Welcome {
        player_id: Uuid,
        character_id: Uuid,
        tick_hz: u32,
    },
    Snapshot(WorldSnapshot),
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorldSnapshot {
    pub server_tick: u64,
    pub players: Vec<PlayerSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerSnapshot {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub tile: TilePos,
}

#[derive(Debug)]
pub enum GameNetEvent {
    Connecting { url: String, character_id: Uuid },
    Connected,
    Welcome {
        player_id: Uuid,
        character_id: Uuid,
        tick_hz: u32,
    },
    Snapshot {
        server_tick: u64,
        players: usize,
        self_tile: Option<TilePos>,
    },
    SentMove { tile: TilePos },
    Error(String),
    Disconnected,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct GameNetStatus {
    pub connecting_url: Option<String>,
    pub connected: bool,
    pub player_id: Option<Uuid>,
    pub character_id: Option<Uuid>,
    pub tick_hz: Option<u32>,
    pub last_snapshot_tick: Option<u64>,
    pub last_snapshot_players: usize,
    pub self_tile: Option<TilePos>,
    pub last_move_sent: Option<TilePos>,
    pub last_error: Option<String>,
}

impl GameNetStatus {
    pub fn reset_for_connect(&mut self, url: String, character_id: Uuid) {
        *self = Self {
            connecting_url: Some(url),
            character_id: Some(character_id),
            ..default()
        };
    }
}

#[derive(Resource)]
pub struct GameNetRuntime {
    pub tx: Sender<GameNetEvent>,
    pub rx: Mutex<Receiver<GameNetEvent>>,
    command_tx: Mutex<Option<Sender<GameNetCommand>>>,
}

impl Default for GameNetRuntime {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
            command_tx: Mutex::new(None),
        }
    }
}

impl GameNetRuntime {
    fn set_command_sender(&self, tx: Sender<GameNetCommand>) {
        *self.command_tx.lock().unwrap() = Some(tx);
    }

    fn clear_command_sender(&self) {
        *self.command_tx.lock().unwrap() = None;
    }

    fn send_command(&self, cmd: GameNetCommand) -> Result<(), String> {
        let Some(tx) = self.command_tx.lock().unwrap().clone() else {
            return Err("game websocket is not ready for commands".to_string());
        };

        tx.send(cmd)
            .map_err(|_| "game websocket command channel is closed".to_string())
    }
}

#[derive(Component)]
pub struct GameNetDebugRoot;

#[derive(Component)]
pub struct GameNetDebugText;

pub fn spawn_game_ws(
    game_net: &mut GameNetRuntime,
    server_base_url: String,
    token: String,
    character_id: Uuid,
) {
    let url = ws_url_from_base(&server_base_url);
    let tx = game_net.tx.clone();
    let (cmd_tx, cmd_rx) = mpsc::channel::<GameNetCommand>();
    game_net.set_command_sender(cmd_tx);

    let _ = tx.send(GameNetEvent::Connecting {
        url: url.clone(),
        character_id,
    });

    thread::spawn(move || {
        if let Err(e) = run_game_ws(url, token, character_id, tx.clone(), cmd_rx) {
            let _ = tx.send(GameNetEvent::Error(e));
        }
        let _ = tx.send(GameNetEvent::Disconnected);
    });
}

fn run_game_ws(
    url: String,
    token: String,
    character_id: Uuid,
    tx: Sender<GameNetEvent>,
    cmd_rx: Receiver<GameNetCommand>,
) -> Result<(), String> {
    let mut request = url
        .clone()
        .into_client_request()
        .map_err(|e| format!("game ws request build failed: {e}"))?;

    let auth = format!("Bearer {token}");
    let auth_value = tungstenite::http::HeaderValue::from_str(&auth)
        .map_err(|e| format!("game ws auth header failed: {e}"))?;
    request.headers_mut().insert("Authorization", auth_value);

    let (mut socket, _response) = connect(request).map_err(|e| format!("game ws connect failed: {e}"))?;

    let _ = tx.send(GameNetEvent::Connected);

    let join = ClientMsg::JoinWorld { character_id };
    let join_json = serde_json::to_string(&join)
        .map_err(|e| format!("game ws join serialize failed: {e}"))?;

    socket
        .send(Message::Text(join_json))
        .map_err(|e| format!("game ws join send failed: {e}"))?;

    let mut player_id: Option<Uuid> = None;

    loop {
        drain_game_commands(&mut socket, &cmd_rx, &tx)?;

        let msg = match socket.read() {
            Ok(m) => m,
            Err(e) => return Err(format!("game ws read failed: {e}")),
        };

        match msg {
            Message::Text(txt) => {
                let parsed: Result<ServerMsg, _> = serde_json::from_str(&txt);
                match parsed {
                    Ok(ServerMsg::Pong) => {}
                    Ok(ServerMsg::Welcome {
                        player_id: pid,
                        character_id,
                        tick_hz,
                    }) => {
                        player_id = Some(pid);
                        let _ = tx.send(GameNetEvent::Welcome {
                            player_id: pid,
                            character_id,
                            tick_hz,
                        });
                    }
                    Ok(ServerMsg::Snapshot(snap)) => {
                        let self_tile = player_id.and_then(|pid| {
                            snap.players.iter().find(|p| p.player_id == pid).map(|p| p.tile)
                        });

                        let _ = tx.send(GameNetEvent::Snapshot {
                            server_tick: snap.server_tick,
                            players: snap.players.len(),
                            self_tile,
                        });
                    }
                    Ok(ServerMsg::Error { message }) => {
                        let _ = tx.send(GameNetEvent::Error(message));
                    }
                    Err(e) => {
                        let _ = tx.send(GameNetEvent::Error(format!(
                            "game ws message parse failed: {e}"
                        )));
                    }
                }
            }
            Message::Close(_) => return Ok(()),
            _ => {}
        }
    }
}

fn drain_game_commands<S: std::io::Read + std::io::Write>(
    socket: &mut tungstenite::WebSocket<S>,
    cmd_rx: &Receiver<GameNetCommand>,
    tx: &Sender<GameNetEvent>,
) -> Result<(), String> {
    while let Ok(cmd) = cmd_rx.try_recv() {
        match cmd {
            GameNetCommand::MoveTo { tile } => {
                let msg = ClientMsg::MoveTo { tile };
                let json = serde_json::to_string(&msg)
                    .map_err(|e| format!("game ws move serialize failed: {e}"))?;

                socket
                    .send(Message::Text(json))
                    .map_err(|e| format!("game ws move send failed: {e}"))?;

                let _ = tx.send(GameNetEvent::SentMove { tile });
            }
        }
    }

    Ok(())
}

fn ws_url_from_base(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    let ws_base = if let Some(rest) = trimmed.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        format!("ws://{trimmed}")
    };

    format!("{ws_base}/v1/game/ws")
}

pub fn pump_game_net_results(game_net: Res<GameNetRuntime>, mut status: ResMut<GameNetStatus>) {
    loop {
        let msg = {
            let rx = game_net.rx.lock().unwrap();
            rx.try_recv()
        };

        let Ok(msg) = msg else { break };

        match msg {
            GameNetEvent::Connecting { url, character_id } => {
                status.reset_for_connect(url.clone(), character_id);
                info!("game net connecting url={} character_id={}", url, character_id);
            }
            GameNetEvent::Connected => {
                status.connected = true;
                status.last_error = None;
                info!("game net connected");
            }
            GameNetEvent::Welcome {
                player_id,
                character_id,
                tick_hz,
            } => {
                status.connected = true;
                status.player_id = Some(player_id);
                status.character_id = Some(character_id);
                status.tick_hz = Some(tick_hz);
                status.last_error = None;

                info!(
                    "game net welcome player_id={} character_id={} tick_hz={}",
                    player_id, character_id, tick_hz
                );
            }
            GameNetEvent::Snapshot {
                server_tick,
                players,
                self_tile,
            } => {
                status.last_snapshot_tick = Some(server_tick);
                status.last_snapshot_players = players;
                status.self_tile = self_tile;
                debug!(
                    "game net snapshot tick={} players={} self_tile={:?}",
                    server_tick, players, self_tile
                );
            }
            GameNetEvent::SentMove { tile } => {
                status.last_move_sent = Some(tile);
                status.last_error = None;
                info!("game net sent move target tile={},{}", tile.x, tile.y);
            }
            GameNetEvent::Error(msg) => {
                status.last_error = Some(msg.clone());
                warn!("game net error: {}", msg);
            }
            GameNetEvent::Disconnected => {
                status.connected = false;
                warn!("game net disconnected");
            }
        }
    }
}

pub fn send_move_target_on_right_click(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    game_net: Res<GameNetRuntime>,
    mut status: ResMut<GameNetStatus>,
) {
    if !buttons.just_pressed(MouseButton::Right) {
        return;
    }

    let Ok(window) = windows.single() else {
        status.last_error = Some("could not find primary window for move target".to_string());
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        status.last_error = Some("right-click move ignored: cursor was outside the window".to_string());
        return;
    };

    let Ok((camera, camera_transform)) = cameras.single() else {
        status.last_error = Some("could not find world camera for move target".to_string());
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        status.last_error = Some("could not convert cursor to world position".to_string());
        return;
    };

    let tile = world_to_tile(world_pos);

    match game_net.send_command(GameNetCommand::MoveTo { tile }) {
        Ok(()) => {
            status.last_move_sent = Some(tile);
            status.last_error = None;
        }
        Err(e) => {
            status.last_error = Some(e);
        }
    }
}

pub fn spawn_game_net_debug_overlay(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/ui.ttf");

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                width: Val::Px(430.0),
                height: Val::Auto,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.015, 0.02, 0.82)),
            GameNetDebugRoot,
            Name::new("game_net_debug_overlay"),
        ))
        .id();

    let text = commands
        .spawn((
            Text::new("Game Runtime: starting..."),
            TextFont {
                font,
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.86, 0.92, 1.0)),
            GameNetDebugText,
            Name::new("game_net_debug_text"),
        ))
        .id();

    commands.entity(root).add_child(text);
}

pub fn sync_game_net_debug_overlay(
    status: Res<GameNetStatus>,
    mut q: Query<&mut Text, With<GameNetDebugText>>,
) {
    if !status.is_changed() {
        return;
    }

    let Ok(mut text) = q.single_mut() else {
        return;
    };

    let connection = if status.connected {
        "Connected"
    } else if status.connecting_url.is_some() {
        "Connecting / Disconnected"
    } else {
        "Not started"
    };

    let player_id = status
        .player_id
        .map(short_uuid)
        .unwrap_or_else(|| "—".to_string());

    let character_id = status
        .character_id
        .map(short_uuid)
        .unwrap_or_else(|| "—".to_string());

    let tick_hz = status
        .tick_hz
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string());

    let snapshot_tick = status
        .last_snapshot_tick
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string());

    let tile = status
        .self_tile
        .map(|t| format!("{}, {}", t.x, t.y))
        .unwrap_or_else(|| "—".to_string());

    let last_move = status
        .last_move_sent
        .map(|t| format!("{}, {}", t.x, t.y))
        .unwrap_or_else(|| "—".to_string());

    let error = status.last_error.as_deref().unwrap_or("—");

    text.0 = format!(
        "Game Runtime\n         Connection: {connection}\n         Player: {player_id}\n         Character: {character_id}\n         Tick Hz: {tick_hz}\n         Snapshot Tick: {snapshot_tick}\n         Snapshot Players: {}\n         Server Tile: {tile}\n         Last Move Sent: {last_move}\n         Last Error: {error}",
        status.last_snapshot_players,
    );
}

pub fn despawn_game_net_debug_overlay(
    mut commands: Commands,
    game_net: Res<GameNetRuntime>,
    roots: Query<Entity, With<GameNetDebugRoot>>,
) {
    game_net.clear_command_sender();

    for root in roots.iter() {
        if let Ok(mut entity) = commands.get_entity(root) {
            entity.despawn();
        }
    }
}

fn short_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
