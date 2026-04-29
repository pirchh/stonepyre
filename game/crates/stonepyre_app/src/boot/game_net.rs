use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::thread;
use tungstenite::{client::IntoClientRequest, connect, Error as WsError, Message};
use uuid::Uuid;

use stonepyre_engine::plugins::interaction::{IntentMsg, Target, Verb};
use stonepyre_engine::plugins::world::{
    player_feet_world, Player, TilePath, FOOT_OFFSET_Y,
};
use stonepyre_world::{tile_to_world_center, world_to_tile, TilePos};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum ClientMsg {
    Ping,
    JoinWorld { character_id: Uuid },
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

    Error {
        message: String,
    },
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
        server_tile: Option<TilePos>,
    },
    MoveSent { tile: TilePos },
    Error(String),
    Disconnected,
}

#[derive(Debug)]
pub enum GameNetCommand {
    MoveTo { tile: TilePos },
}

#[derive(Resource, Debug, Clone)]
pub struct GameNetStatus {
    pub connected: bool,
    pub connecting: bool,
    pub player_id: Option<Uuid>,
    pub character_id: Option<Uuid>,
    pub tick_hz: Option<u32>,
    pub server_tick: Option<u64>,
    pub snapshot_players: usize,
    pub server_tile: Option<TilePos>,
    pub local_tile: Option<TilePos>,
    pub drift_tiles: Option<i32>,
    pub last_move_sent: Option<TilePos>,
    pub last_error: Option<String>,
    pub correction_count: u64,
}

impl Default for GameNetStatus {
    fn default() -> Self {
        Self {
            connected: false,
            connecting: false,
            player_id: None,
            character_id: None,
            tick_hz: None,
            server_tick: None,
            snapshot_players: 0,
            server_tile: None,
            local_tile: None,
            drift_tiles: None,
            last_move_sent: None,
            last_error: None,
            correction_count: 0,
        }
    }
}

/// Runtime bridge for the server-side game websocket.
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

#[derive(Component)]
pub struct GameNetOverlayRoot;

#[derive(Component)]
pub struct GameNetOverlayText;

pub fn spawn_game_ws(
    game_net: &mut GameNetRuntime,
    status: &mut GameNetStatus,
    server_base_url: String,
    token: String,
    character_id: Uuid,
) {
    let url = ws_url_from_base(&server_base_url);
    let tx = game_net.tx.clone();
    let (cmd_tx, cmd_rx) = mpsc::channel::<GameNetCommand>();

    *game_net.command_tx.lock().unwrap() = Some(cmd_tx);

    status.connected = false;
    status.connecting = true;
    status.character_id = Some(character_id);
    status.player_id = None;
    status.server_tick = None;
    status.snapshot_players = 0;
    status.server_tile = None;
    status.local_tile = None;
    status.drift_tiles = None;
    status.last_move_sent = None;
    status.last_error = None;

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

pub fn send_move_to_server(game_net: &GameNetRuntime, tile: TilePos) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else {
        return false;
    };

    tx.send(GameNetCommand::MoveTo { tile }).is_ok()
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

    let (mut socket, _response) = connect(request)
        .map_err(|e| format!("game ws connect failed: {e}"))?;

    // Keep the websocket responsive to client commands while we wait for server messages.
    // tungstenite wraps the TcpStream in MaybeTlsStream; in local/dev ws:// mode this is Plain.
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        if let Err(e) = stream.set_read_timeout(Some(std::time::Duration::from_millis(50))) {
            let _ = tx.send(GameNetEvent::Error(format!(
                "game ws read timeout setup failed: {e}"
            )));
        }
    }

    let _ = tx.send(GameNetEvent::Connected);

    let join = ClientMsg::JoinWorld { character_id };
    let join_json = serde_json::to_string(&join)
        .map_err(|e| format!("game ws join serialize failed: {e}"))?;

    socket
        .send(Message::Text(join_json))
        .map_err(|e| format!("game ws join send failed: {e}"))?;

    let mut player_id: Option<Uuid> = None;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                GameNetCommand::MoveTo { tile } => {
                    let msg = ClientMsg::MoveTo { tile };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws move serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws move send failed: {e}"))?;
                    let _ = tx.send(GameNetEvent::MoveSent { tile });
                }
            }
        }

        let msg = match socket.read() {
            Ok(m) => m,
            Err(WsError::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
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
                        let server_tile = player_id.and_then(|pid| {
                            snap.players
                                .iter()
                                .find(|p| p.player_id == pid)
                                .map(|p| p.tile)
                        });

                        let _ = tx.send(GameNetEvent::Snapshot {
                            server_tick: snap.server_tick,
                            players: snap.players.len(),
                            server_tile,
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

pub fn pump_game_net_results(
    game_net: Res<GameNetRuntime>,
    mut status: ResMut<GameNetStatus>,
) {
    loop {
        let msg = {
            let rx = game_net.rx.lock().unwrap();
            rx.try_recv()
        };

        let Ok(msg) = msg else { break };

        match msg {
            GameNetEvent::Connecting { url, character_id } => {
                status.connecting = true;
                status.connected = false;
                status.character_id = Some(character_id);
                status.last_error = None;
                info!("game net connecting url={} character_id={}", url, character_id);
            }
            GameNetEvent::Connected => {
                status.connecting = false;
                status.connected = true;
                status.last_error = None;
                info!("game net connected");
            }
            GameNetEvent::Welcome {
                player_id,
                character_id,
                tick_hz,
            } => {
                status.player_id = Some(player_id);
                status.character_id = Some(character_id);
                status.tick_hz = Some(tick_hz);
                status.connected = true;
                status.connecting = false;
                info!(
                    "game net welcome player_id={} character_id={} tick_hz={}",
                    player_id, character_id, tick_hz
                );
            }
            GameNetEvent::Snapshot {
                server_tick,
                players,
                server_tile,
            } => {
                status.server_tick = Some(server_tick);
                status.snapshot_players = players;
                if let Some(tile) = server_tile {
                    status.server_tile = Some(tile);
                }
                debug!("game net snapshot tick={} players={}", server_tick, players);
            }
            GameNetEvent::MoveSent { tile } => {
                status.last_move_sent = Some(tile);
                info!("game net sent move target tile={},{}", tile.x, tile.y);
            }
            GameNetEvent::Error(msg) => {
                status.last_error = Some(msg.clone());
                warn!("game net error: {}", msg);
            }
            GameNetEvent::Disconnected => {
                status.connected = false;
                status.connecting = false;
                warn!("game net disconnected");
            }
        }
    }
}

/// Send authoritative movement commands only after the actual WalkHere intent is chosen.
/// Right-click still only opens the context menu.
pub fn send_walk_intents_to_server_runtime(
    mut intents: MessageReader<IntentMsg>,
    game_net: Res<GameNetRuntime>,
) {
    for ev in intents.read() {
        if ev.intent.verb != Verb::WalkHere {
            continue;
        }

        let Target::Tile(tile) = ev.intent.target else {
            continue;
        };

        if !send_move_to_server(&game_net, tile) {
            warn!("game net move target dropped; websocket is not ready");
        }
    }
}

/// Track the visible player tile, compare it to the authoritative server tile,
/// and only hard-correct if the local player drifts too far away.
///
/// The client still predicts immediately by using the existing local TilePath. This system is
/// reconciliation, not the only movement driver.
pub fn reconcile_local_player_to_server(
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<(&mut Transform, &mut TilePath), With<Player>>,
) {
    let Ok((mut xform, mut path)) = player_q.single_mut() else {
        status.local_tile = None;
        status.drift_tiles = None;
        return;
    };

    let local_tile = world_to_tile(player_feet_world(&xform));
    status.local_tile = Some(local_tile);

    let Some(server_tile) = status.server_tile else {
        status.drift_tiles = None;
        return;
    };

    let drift = (local_tile.x - server_tile.x).abs() + (local_tile.y - server_tile.y).abs();
    status.drift_tiles = Some(drift);

    // Small differences are normal while prediction and snapshots are in flight.
    // If we drift too far, resync locally to the authoritative tile.
    if drift >= 3 {
        let center = tile_to_world_center(server_tile);
        xform.translation.x = center.x;
        xform.translation.y = center.y + FOOT_OFFSET_Y;
        path.tiles.clear();
        status.correction_count += 1;
        warn!(
            "game net corrected local player to server tile {},{} drift={}",
            server_tile.x, server_tile.y, drift
        );
    }
}

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
