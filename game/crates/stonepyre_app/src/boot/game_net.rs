use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use std::thread;
use tungstenite::{client::IntoClientRequest, connect, Message};
use uuid::Uuid;

use stonepyre_world::TilePos;

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
    },
    Error(String),
    Disconnected,
}

/// Small runtime bridge for the server-side game websocket.
///
/// This is intentionally minimal. It proves that the client can join the server-side
/// GameSim and receive server snapshots. Once that path is stable, movement/input can
/// start sending ClientMsg::MoveTo through this same bridge.
#[derive(Resource)]
pub struct GameNetRuntime {
    pub tx: Sender<GameNetEvent>,
    pub rx: Mutex<Receiver<GameNetEvent>>,
}

impl Default for GameNetRuntime {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
        }
    }
}

pub fn spawn_game_ws(
    game_net: &mut GameNetRuntime,
    server_base_url: String,
    token: String,
    character_id: Uuid,
) {
    let url = ws_url_from_base(&server_base_url);
    let tx = game_net.tx.clone();

    let _ = tx.send(GameNetEvent::Connecting {
        url: url.clone(),
        character_id,
    });

    thread::spawn(move || {
        if let Err(e) = run_game_ws(url, token, character_id, tx.clone()) {
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

    let _ = tx.send(GameNetEvent::Connected);

    let join = ClientMsg::JoinWorld { character_id };
    let join_json = serde_json::to_string(&join)
        .map_err(|e| format!("game ws join serialize failed: {e}"))?;

    socket
        .send(Message::Text(join_json))
        .map_err(|e| format!("game ws join send failed: {e}"))?;

    loop {
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
                        player_id,
                        character_id,
                        tick_hz,
                    }) => {
                        let _ = tx.send(GameNetEvent::Welcome {
                            player_id,
                            character_id,
                            tick_hz,
                        });
                    }
                    Ok(ServerMsg::Snapshot(snap)) => {
                        let _ = tx.send(GameNetEvent::Snapshot {
                            server_tick: snap.server_tick,
                            players: snap.players.len(),
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

pub fn pump_game_net_results(game_net: Res<GameNetRuntime>) {
    loop {
        let msg = {
            let rx = game_net.rx.lock().unwrap();
            rx.try_recv()
        };

        let Ok(msg) = msg else { break };

        match msg {
            GameNetEvent::Connecting { url, character_id } => {
                info!("game net connecting url={} character_id={}", url, character_id);
            }
            GameNetEvent::Connected => {
                info!("game net connected");
            }
            GameNetEvent::Welcome {
                player_id,
                character_id,
                tick_hz,
            } => {
                info!(
                    "game net welcome player_id={} character_id={} tick_hz={}",
                    player_id, character_id, tick_hz
                );
            }
            GameNetEvent::Snapshot {
                server_tick,
                players,
            } => {
                // Keep this at debug so normal logs do not get spammed every snapshot.
                debug!("game net snapshot tick={} players={}", server_tick, players);
            }
            GameNetEvent::Error(msg) => {
                warn!("game net error: {}", msg);
            }
            GameNetEvent::Disconnected => {
                warn!("game net disconnected");
            }
        }
    }
}
