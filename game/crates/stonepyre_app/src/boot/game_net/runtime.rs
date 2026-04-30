use bevy::prelude::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use tungstenite::{client::IntoClientRequest, connect, Error as WsError, Message};
use uuid::Uuid;

use stonepyre_engine::plugins::interaction::{IntentMsg, Target, Verb};
use stonepyre_engine::plugins::world::GridPos;
use stonepyre_world::TilePos;

use super::protocol::{
    ActionState,
    ClientMsg,
    InteractionAction,
    InteractionTarget,
    NetPlayerSnapshot,
    ServerMsg,
};
use super::status::{GameNetCommand, GameNetEvent, GameNetRuntime, GameNetStatus};

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
    status.latest_players.clear();
    status.harvest_nodes.clear();
    status.server_tile = None;
    status.server_next_tile = None;
    status.server_goal = None;
    status.server_moving = false;
    status.server_action = None;
    status.inventory_items.clear();
    status.inventory_dirty = true;
    status.local_tile = None;
    status.drift_tiles = None;
    status.last_move_sent = None;
    status.action_marker_target = None;
    status.last_error = None;
    status.remote_player_count = 0;

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

pub fn send_interaction_to_server(
    game_net: &GameNetRuntime,
    action: InteractionAction,
    target: InteractionTarget,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else {
        return false;
    };

    tx.send(GameNetCommand::Interact { action, target }).is_ok()
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
                GameNetCommand::Interact { action, target } => {
                    let msg = ClientMsg::Interact {
                        action,
                        target: target.clone(),
                    };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws interaction serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws interaction send failed: {e}"))?;

                    if let InteractionTarget::Tile(tile) = target {
                        if action == InteractionAction::WalkHere {
                            let _ = tx.send(GameNetEvent::MoveSent { tile });
                        }
                    }
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
                        let players: Vec<NetPlayerSnapshot> = snap
                            .players
                            .iter()
                            .map(|p| NetPlayerSnapshot {
                                player_id: p.player_id,
                                character_id: p.character_id,
                                tile: p.tile,
                                next_tile: p.next_tile,
                                goal: p.goal,
                                moving: p.moving,
                                action: p.action.clone(),
                            })
                            .collect();

                        let local_player = player_id
                            .and_then(|pid| snap.players.iter().find(|p| p.player_id == pid));

                        let server_tile = local_player.map(|p| p.tile);
                        let server_next_tile = local_player.and_then(|p| p.next_tile);
                        let server_goal = local_player.and_then(|p| p.goal);
                        let server_moving = local_player.map(|p| p.moving).unwrap_or(false);
                        let server_action = local_player.and_then(|p| p.action.clone());

                        let _ = tx.send(GameNetEvent::Snapshot {
                            server_tick: snap.server_tick,
                            players,
                            harvest_nodes: snap.harvest_nodes,
                            server_tile,
                            server_next_tile,
                            server_goal,
                            server_moving,
                            server_action,
                        });
                    }
                    Ok(ServerMsg::InteractionAck {
                        accepted,
                        action,
                        target,
                        message,
                    }) => {
                        let _ = tx.send(GameNetEvent::InteractionAck {
                            accepted,
                            action,
                            target,
                            message,
                        });
                    }
                    Ok(ServerMsg::ActionState {
                        player_id,
                        action,
                        target,
                        state,
                        message,
                    }) => {
                        let _ = tx.send(GameNetEvent::ActionState {
                            player_id,
                            action,
                            target,
                            state,
                            message,
                        });
                    }
                    Ok(ServerMsg::InventorySnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::InventorySnapshot(snapshot));
                    }
                    Ok(ServerMsg::InventoryDelta(delta)) => {
                        let _ = tx.send(GameNetEvent::InventoryDelta(delta));
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
                harvest_nodes,
                server_tile,
                server_next_tile,
                server_goal,
                server_moving,
                server_action,
            } => {
                status.server_tick = Some(server_tick);
                status.snapshot_players = players.len();
                status.latest_players = players;
                status.harvest_nodes = harvest_nodes;
                status.server_moving = server_moving;
                status.server_next_tile = server_next_tile;
                status.server_goal = server_goal;
                status.server_action = server_action;
                if let Some(tile) = server_tile {
                    status.server_tile = Some(tile);
                }
                debug!(
                    "game net snapshot tick={} players={} harvest_nodes={} server_tile={:?} next_tile={:?} goal={:?} moving={}",
                    server_tick,
                    status.snapshot_players,
                    status.harvest_nodes.len(),
                    status.server_tile,
                    status.server_next_tile,
                    status.server_goal,
                    status.server_moving
                );
            }
            GameNetEvent::MoveSent { tile } => {
                status.last_move_sent = Some(tile);
                status.action_marker_target = None;
                status.server_action = None;
                info!("game net sent move target tile={},{}", tile.x, tile.y);
            }
            GameNetEvent::InteractionAck {
                accepted,
                action,
                target,
                message,
            } => {
                if accepted {
                    info!(
                        "game ws interaction accepted action={:?} target={:?}: {}",
                        action, target, message
                    );

                    if action == InteractionAction::ChopDown {
                        if let InteractionTarget::Tile(tile) = target {
                            status.action_marker_target = Some(tile);
                            status.last_error = None;
                        }
                    }
                } else {
                    status.last_error = Some(message.clone());
                    warn!("game net interaction rejected action={:?} target={:?}: {}", action, target, message);
                }
            }
            GameNetEvent::ActionState {
                player_id,
                action,
                target,
                state,
                message,
            } => {
                info!(
                    "game net action state player_id={} action={:?} target={:?} state={:?}: {}",
                    player_id, action, target, state, message
                );

                if status.player_id == Some(player_id) {
                    match state {
                        ActionState::Queued | ActionState::MovingToRange | ActionState::Active => {
                            if let InteractionTarget::Tile(tile) = target {
                                status.action_marker_target = Some(tile);
                            }
                        }
                        ActionState::Cancelled | ActionState::Complete | ActionState::Rejected => {
                            status.server_action = None;
                            status.action_marker_target = None;
                        }
                    }
                }
            }
            GameNetEvent::InventorySnapshot(snapshot) => {
                info!(
                    "game net inventory snapshot character_id={} items={}",
                    snapshot.character_id,
                    snapshot.items.len()
                );
                status.inventory_items = snapshot.items;
                status.inventory_dirty = true;
            }
            GameNetEvent::InventoryDelta(delta) => {
                info!(
                    "game net inventory delta character_id={} item_id={} delta={} new_quantity={}",
                    delta.character_id,
                    delta.item_id,
                    delta.quantity_delta,
                    delta.new_quantity
                );

                if status.character_id != Some(delta.character_id) {
                    continue;
                }

                let item_id = delta.item_id.clone();
                if delta.new_quantity <= 0 {
                    status.inventory_items.retain(|item| item.item_id != item_id.as_str());
                } else if let Some(item) = status
                    .inventory_items
                    .iter_mut()
                    .find(|item| item.item_id == item_id.as_str())
                {
                    item.quantity = delta.new_quantity;
                } else {
                    status.inventory_items.push(super::protocol::InventoryItemSnapshot {
                        item_id,
                        quantity: delta.new_quantity,
                    });
                }

                status.inventory_items.sort_by(|a, b| a.item_id.cmp(&b.item_id));
                status.inventory_dirty = true;
            }
            GameNetEvent::Error(msg) => {
                status.last_error = Some(msg.clone());
                warn!("game net error: {}", msg);
            }
            GameNetEvent::Disconnected => {
                status.connected = false;
                status.connecting = false;
                status.latest_players.clear();
                status.harvest_nodes.clear();
                status.server_next_tile = None;
                status.server_goal = None;
                status.server_moving = false;
                status.server_action = None;
                status.inventory_items.clear();
                status.inventory_dirty = true;
                status.action_marker_target = None;
                status.remote_player_count = 0;
                warn!("game net disconnected");
            }
        }
    }
}

pub fn send_walk_intents_to_server_runtime(
    mut intents: MessageReader<IntentMsg>,
    game_net: Res<GameNetRuntime>,
    grid_pos_q: Query<&GridPos>,
) {
    for ev in intents.read() {
        match ev.intent.verb {
            Verb::WalkHere => {
                let Target::Tile(tile) = ev.intent.target else {
                    continue;
                };

                if !send_move_to_server(&game_net, tile) {
                    warn!("game net move target dropped; websocket is not ready");
                }
            }
            Verb::ChopDown => {
                let Some(tile) = intent_target_tile(ev.intent.target, &grid_pos_q) else {
                    warn!("game net chopdown target dropped; target tile could not be resolved");
                    continue;
                };

                if !send_interaction_to_server(
                    &game_net,
                    InteractionAction::ChopDown,
                    InteractionTarget::Tile(tile),
                ) {
                    warn!("game net chopdown target dropped; websocket is not ready");
                }
            }
            Verb::TalkTo | Verb::Examine => {}
        }
    }
}

fn intent_target_tile(target: Target, grid_pos_q: &Query<&GridPos>) -> Option<TilePos> {
    match target {
        Target::Tile(tile) => Some(tile),
        Target::Entity(entity) => grid_pos_q.get(entity).ok().map(|gp| gp.0),
    }
}
