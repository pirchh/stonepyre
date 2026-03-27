use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{error::ApiError, http::middleware::AuthContext, state::AppState};
use crate::game::protocol::{ClientMsg, ServerMsg};

pub async fn game_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, ApiError> {
    // ctx validated by middleware; accept upgrade
    Ok(ws.on_upgrade(move |socket| handle_socket(state, ctx, socket)))
}

async fn handle_socket(state: AppState, _ctx: AuthContext, socket: WebSocket) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Outgoing queue so multiple producers can send without cloning ws_tx
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMsg>();

    // Subscribe to server broadcast stream
    let mut broadcast_rx = state.game.hub.subscribe();

    // Until JoinWorld, this is None
    let mut player_id: Option<Uuid> = None;

    // Task: pump outbound messages to websocket
    let write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let Ok(json) = serde_json::to_string(&msg) else { continue; };
            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Task: forward broadcast -> out queue
    let out_tx_broadcast = out_tx.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            let _ = out_tx_broadcast.send(msg);
        }
    });

    // Read client messages
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(txt) => {
                let parsed: Result<ClientMsg, _> = serde_json::from_str(&txt);
                let Ok(cm) = parsed else {
                    let _ = out_tx.send(ServerMsg::Error {
                        message: "bad message json".to_string(),
                    });
                    continue;
                };

                match cm {
                    ClientMsg::Ping => {
                        let _ = out_tx.send(ServerMsg::Pong);
                    }

                    ClientMsg::JoinWorld { character_id } => {
                        let pid = player_id.get_or_insert_with(Uuid::new_v4).to_owned();

                        {
                            let mut sim = state.game.sim.write().await;
                            sim.add_player(pid, character_id);
                        }

                        let tick_hz: u32 = std::env::var("GAME_TICK_HZ")
                            .ok()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(10);

                        let welcome = ServerMsg::Welcome {
                            player_id: pid,
                            character_id,
                            tick_hz,
                        };

                        let _ = out_tx.send(welcome);
                    }

                    ClientMsg::MoveTo { tile } => {
                        if let Some(pid) = player_id {
                            let mut sim = state.game.sim.write().await;
                            sim.set_move_target(pid, tile);
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // cleanup on disconnect
    if let Some(pid) = player_id {
        let mut sim = state.game.sim.write().await;
        sim.remove_player(pid);
    }

    forward_task.abort();
    write_task.abort();
}