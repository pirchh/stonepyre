use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    error::ApiError,
    game::{
        protocol::{ActionState, ClientMsg, InteractionAction, InteractionTarget, ServerMsg},
        ActiveCharacterJoinError,
    },
    http::middleware::AuthContext,
    state::AppState,
};

pub async fn game_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, ApiError> {
    Ok(ws.on_upgrade(move |socket| handle_socket(state, ctx, socket)))
}

async fn handle_socket(state: AppState, ctx: AuthContext, socket: WebSocket) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<ServerMsg>();
    let mut broadcast_rx = state.game.hub.subscribe();

    let mut player_id: Option<Uuid> = None;
    let mut character_id: Option<Uuid> = None;

    let write_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let Ok(json) = serde_json::to_string(&msg) else {
                continue;
            };

            if ws_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let out_tx_broadcast = out_tx.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(msg) = broadcast_rx.recv().await {
            let _ = out_tx_broadcast.send(msg);
        }
    });

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
                    ClientMsg::JoinWorld {
                        character_id: requested_character_id,
                    } => {
                        if player_id.is_some() {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "this websocket has already joined the world".to_string(),
                            });
                            continue;
                        }

                        let owns_character = match account_owns_character(
                            &state,
                            ctx.account_id,
                            requested_character_id,
                        )
                        .await
                        {
                            Ok(owns) => owns,
                            Err(e) => {
                                warn!(
                                    "game ws character ownership check failed account_id={} character_id={} error={:?}",
                                    ctx.account_id, requested_character_id, e
                                );
                                let _ = out_tx.send(ServerMsg::Error {
                                    message: "failed to validate character ownership".to_string(),
                                });
                                continue;
                            }
                        };

                        if !owns_character {
                            warn!(
                                "game ws join rejected: account_id={} does not own character_id={}",
                                ctx.account_id, requested_character_id
                            );
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "you do not own this character".to_string(),
                            });
                            continue;
                        }

                        let requested_player_id = Uuid::new_v4();

                        let reserve_result = {
                            let mut sessions = state.game.sessions.write().await;
                            sessions.reserve_character(requested_player_id, requested_character_id)
                        };

                        if let Err(err) = reserve_result {
                            let message = join_error_message(err);
                            warn!("game ws join rejected: {}", message);

                            let _ = out_tx.send(ServerMsg::Error { message });
                            continue;
                        }

                        {
                            let mut sim = state.game.sim.write().await;
                            sim.add_player(requested_player_id, requested_character_id);
                        }

                        player_id = Some(requested_player_id);
                        character_id = Some(requested_character_id);

                        let tick_hz: u32 = std::env::var("GAME_TICK_HZ")
                            .ok()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(10);

                        info!(
                            "game ws joined account_id={} player_id={} character_id={}",
                            ctx.account_id, requested_player_id, requested_character_id
                        );

                        let _ = out_tx.send(ServerMsg::Welcome {
                            player_id: requested_player_id,
                            character_id: requested_character_id,
                            tick_hz,
                        });

                        match crate::game::sim::inventory::load_character_inventory_snapshot(
                            &state.db,
                            requested_character_id,
                        )
                        .await
                        {
                            Ok(snapshot) => {
                                let _ = out_tx.send(ServerMsg::InventorySnapshot(snapshot));
                            }
                            Err(e) => {
                                warn!(
                                    "game ws inventory snapshot failed account_id={} character_id={} error={:?}",
                                    ctx.account_id, requested_character_id, e
                                );
                                let _ = out_tx.send(ServerMsg::Error {
                                    message: "failed to load inventory".to_string(),
                                });
                            }
                        }
                    }
                    ClientMsg::MoveTo { tile } => {
                        if let Some(pid) = player_id {
                            let event = {
                                let mut sim = state.game.sim.write().await;
                                sim.set_move_target(pid, tile)
                            };

                            if let Some(event) = event {
                                let _ = out_tx.send(event);
                            }
                        }
                    }
                    ClientMsg::Interact { action, target } => {
                        let Some(pid) = player_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before sending interactions".to_string(),
                            });
                            continue;
                        };

                        handle_interaction(&state, pid, action, target, &out_tx).await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if let Some(pid) = player_id {
        {
            let mut sim = state.game.sim.write().await;
            sim.remove_player(pid);
        }

        {
            let mut sessions = state.game.sessions.write().await;
            sessions.release_player(pid);
        }

        if let Some(cid) = character_id {
            info!(
                "game ws disconnected account_id={} player_id={} character_id={}",
                ctx.account_id, pid, cid
            );
        } else {
            info!("game ws disconnected account_id={} player_id={}", ctx.account_id, pid);
        }
    }

    forward_task.abort();
    write_task.abort();
}

async fn handle_interaction(
    state: &AppState,
    player_id: Uuid,
    action: InteractionAction,
    target: InteractionTarget,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match (action, target.clone()) {
        (InteractionAction::WalkHere, InteractionTarget::Tile(tile)) => {
            let event = {
                let mut sim = state.game.sim.write().await;
                sim.set_move_target(player_id, tile)
            };

            if let Some(event) = event {
                let _ = out_tx.send(event);
            }

            let _ = out_tx.send(ServerMsg::InteractionAck {
                accepted: true,
                action,
                target,
                message: "walk target accepted".to_string(),
            });
        }
        (InteractionAction::ChopDown, InteractionTarget::Tile(tile)) => {
            let validation = {
                let mut sim = state.game.sim.write().await;
                sim.queue_chop_down(player_id, tile)
            };

            match validation {
                Ok((state, message)) => {
                    let _ = out_tx.send(ServerMsg::InteractionAck {
                        accepted: true,
                        action,
                        target: target.clone(),
                        message: message.clone(),
                    });

                    let _ = out_tx.send(ServerMsg::ActionState {
                        player_id,
                        action,
                        target,
                        state,
                        message,
                    });
                }
                Err(message) => {
                    let _ = out_tx.send(ServerMsg::InteractionAck {
                        accepted: false,
                        action,
                        target: target.clone(),
                        message: message.clone(),
                    });

                    let _ = out_tx.send(ServerMsg::ActionState {
                        player_id,
                        action,
                        target,
                        state: ActionState::Rejected,
                        message,
                    });
                }
            }
        }
    }
}

async fn account_owns_character(
    state: &AppState,
    account_id: Uuid,
    character_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT game.account_owns_character($1::uuid, $2::uuid)
        "#,
    )
    .bind(account_id)
    .bind(character_id)
    .fetch_one(&state.db)
    .await
}

fn join_error_message(err: ActiveCharacterJoinError) -> String {
    match err {
        ActiveCharacterJoinError::CharacterAlreadyActive {
            character_id,
            existing_player_id,
        } => format!(
            "character {} is already active in the world as player {}",
            character_id, existing_player_id
        ),
        ActiveCharacterJoinError::PlayerAlreadyJoined {
            player_id,
            existing_character_id,
        } => format!(
            "player {} has already joined the world as character {}",
            player_id, existing_character_id
        ),
    }
}
