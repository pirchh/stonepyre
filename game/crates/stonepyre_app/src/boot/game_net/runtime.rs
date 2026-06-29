use bevy::prelude::*;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use tungstenite::{client::IntoClientRequest, connect, Error as WsError, Message};
use uuid::Uuid;

use stonepyre_engine::plugins::interaction::{IntentMsg, Target, Verb};
use stonepyre_engine::plugins::movement::StepTo;
use stonepyre_engine::plugins::world::{GridPos, Player, ServerBlockedTiles, TilePath};
use stonepyre_world::TilePos;

use super::protocol::{
    ActionState,
    ClientMsg,
    GroundItemEventKind,
    InteractionAction,
    InteractionTarget,
    NetPlayerSnapshot,
    ServerMsg,
};
use super::status::{GameNetCommand, GameNetEvent, GameNetRuntime, GameNetStatus};

#[derive(Resource, Default, Debug, Clone)]
pub struct PendingGroundItemPickup {
    pub request: Option<PendingGroundItemPickupRequest>,
}

#[derive(Debug, Clone)]
pub struct PendingGroundItemPickupRequest {
    pub ground_item_id: Uuid,
    pub tile: TilePos,
}

/// Booth tile stored when the player clicks a bank. We send a WalkHere to the
/// server immediately, then fire the actual UseBank interaction only once the
/// server-reconciled player position is within 1 Chebyshev tile of the booth.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingBankOpen {
    pub booth_tile: Option<TilePos>,
}

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
    status.ground_items.clear();
    status.ground_items_dirty = true;
    status.server_pos = None;
    status.server_tile = None;
    status.server_next_tile = None;
    status.server_goal = None;
    status.server_moving = false;
    status.server_move_progress = 0.0;
    status.server_action = None;
    status.pending_server_path = None;
    status.pending_path_confirmations = 0;
    status.inventory_slots_total = 20;
    status.inventory_items.clear();
    status.inventory_dirty = true;
    status.bag_slots.clear();
    status.bag_slots_dirty = true;
    status.skill_entries.clear();
    status.skills_dirty = true;
    status.feedback_drops.clear();
    status.local_tile = None;
    status.drift_tiles = None;
    status.last_move_sent = None;
    status.action_marker_target = None;
    status.last_error = None;
    status.remote_player_count = 0;
    status.initial_sync_done = false;

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

pub fn send_drop_item_to_server(
    game_net: &GameNetRuntime,
    slot_idx: usize,
    item_id: String,
    quantity: u32,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else {
        return false;
    };

    tx.send(GameNetCommand::DropItem {
        slot_idx,
        item_id,
        quantity,
    })
    .is_ok()
}

pub fn send_pickup_ground_item_to_server(
    game_net: &GameNetRuntime,
    ground_item_id: Uuid,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else {
        return false;
    };

    tx.send(GameNetCommand::PickupGroundItem { ground_item_id }).is_ok()
}

pub fn send_equip_bag_to_server(
    game_net: &GameNetRuntime,
    inventory_slot_idx: usize,
    bag_slot: u8,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::EquipBag { inventory_slot_idx, bag_slot }).is_ok()
}

pub fn send_unequip_bag_to_server(game_net: &GameNetRuntime, bag_slot: u8) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::UnequipBag { bag_slot }).is_ok()
}

pub fn send_equip_item_to_server(game_net: &GameNetRuntime, inventory_slot_idx: usize, item_id: String) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::EquipItem { inventory_slot_idx, item_id }).is_ok()
}

pub fn send_unequip_item_to_server(game_net: &GameNetRuntime, slot: String) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::UnequipItem { slot }).is_ok()
}

pub fn send_bag_put_item_to_server(
    game_net: &GameNetRuntime,
    bag_slot: u8,
    inventory_slot_idx: usize,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BagPutItem { bag_slot, inventory_slot_idx }).is_ok()
}

pub fn send_bag_take_item_to_server(
    game_net: &GameNetRuntime,
    bag_slot: u8,
    bag_item_slot_idx: usize,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BagTakeItem { bag_slot, bag_item_slot_idx }).is_ok()
}

pub fn send_swap_inv_slots_to_server(
    game_net: &GameNetRuntime,
    from_slot: usize,
    to_slot: usize,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::SwapInvSlots { from_slot, to_slot }).is_ok()
}

pub fn send_bag_move_item_to_server(
    game_net: &GameNetRuntime,
    from_bag_slot: u8,
    from_item_slot: usize,
    to_bag_slot: u8,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BagMoveItem { from_bag_slot, from_item_slot, to_bag_slot }).is_ok()
}

pub fn send_bag_put_item_to_slot_to_server(
    game_net: &GameNetRuntime,
    bag_slot: u8,
    inventory_slot_idx: usize,
    bag_item_slot_idx: usize,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BagPutItemToSlot { bag_slot, inventory_slot_idx, bag_item_slot_idx }).is_ok()
}

pub fn send_bag_take_item_to_slot_to_server(
    game_net: &GameNetRuntime,
    bag_slot: u8,
    bag_item_slot_idx: usize,
    inv_slot_idx: usize,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BagTakeItemToSlot { bag_slot, bag_item_slot_idx, inv_slot_idx }).is_ok()
}

pub fn send_bank_deposit_to_server(
    game_net: &GameNetRuntime,
    inv_slot_idx: usize,
    item_id: String,
    quantity: u32,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BankDeposit { inv_slot_idx, item_id, quantity }).is_ok()
}

pub fn send_bank_withdraw_to_server(
    game_net: &GameNetRuntime,
    tab_idx: u8,
    slot_idx: usize,
    item_id: String,
    quantity: u32,
) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BankWithdraw { tab_idx, slot_idx, item_id, quantity }).is_ok()
}

pub fn send_bank_deposit_all_to_server(game_net: &GameNetRuntime) -> bool {
    let guard = game_net.command_tx.lock().unwrap();
    let Some(tx) = guard.as_ref() else { return false; };
    tx.send(GameNetCommand::BankDepositAll).is_ok()
}

pub fn send_bank_create_tab_to_server(game_net: &GameNetRuntime, display_name: String) -> bool {
    let cmd = GameNetCommand::BankCreateTab { display_name };
    let guard = game_net.command_tx.lock().unwrap();
    guard.as_ref().map(|tx| tx.send(cmd).is_ok()).unwrap_or(false)
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
                GameNetCommand::MoveDir { dx, dy, seq } => {
                    let msg = ClientMsg::MoveDir { dx, dy, seq };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws move_dir serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws move_dir send failed: {e}"))?;
                }
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
                GameNetCommand::DropItem {
                    slot_idx,
                    item_id,
                    quantity,
                } => {
                    let msg = ClientMsg::DropItem {
                        slot_idx,
                        item_id,
                        quantity,
                    };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws drop item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws drop item send failed: {e}"))?;
                }
                GameNetCommand::PickupGroundItem { ground_item_id } => {
                    let msg = ClientMsg::PickupGroundItem { ground_item_id };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws pickup ground item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws pickup ground item send failed: {e}"))?;
                }
                GameNetCommand::EquipBag { inventory_slot_idx, bag_slot } => {
                    let msg = ClientMsg::EquipBag { inventory_slot_idx, bag_slot };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws equip bag serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws equip bag send failed: {e}"))?;
                }
                GameNetCommand::UnequipBag { bag_slot } => {
                    let msg = ClientMsg::UnequipBag { bag_slot };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws unequip bag serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws unequip bag send failed: {e}"))?;
                }
                GameNetCommand::EquipItem { inventory_slot_idx, item_id } => {
                    let msg = ClientMsg::EquipItem { inventory_slot_idx, item_id };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws equip item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws equip item send failed: {e}"))?;
                }
                GameNetCommand::UnequipItem { slot } => {
                    let msg = ClientMsg::UnequipItem { slot };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws unequip item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws unequip item send failed: {e}"))?;
                }
                GameNetCommand::BagPutItem { bag_slot, inventory_slot_idx } => {
                    let msg = ClientMsg::BagPutItem { bag_slot, inventory_slot_idx };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bag put item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bag put item send failed: {e}"))?;
                }
                GameNetCommand::BagTakeItem { bag_slot, bag_item_slot_idx } => {
                    let msg = ClientMsg::BagTakeItem { bag_slot, bag_item_slot_idx };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bag take item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bag take item send failed: {e}"))?;
                }
                GameNetCommand::SwapInvSlots { from_slot, to_slot } => {
                    let msg = ClientMsg::SwapInvSlots { from_slot, to_slot };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws swap inv slots serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws swap inv slots send failed: {e}"))?;
                }
                GameNetCommand::BagMoveItem { from_bag_slot, from_item_slot, to_bag_slot } => {
                    let msg = ClientMsg::BagMoveItem { from_bag_slot, from_item_slot, to_bag_slot };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bag move item serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bag move item send failed: {e}"))?;
                }
                GameNetCommand::BagPutItemToSlot { bag_slot, inventory_slot_idx, bag_item_slot_idx } => {
                    let msg = ClientMsg::BagPutItemToSlot { bag_slot, inventory_slot_idx, bag_item_slot_idx };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bag put item to slot serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bag put item to slot send failed: {e}"))?;
                }
                GameNetCommand::BagTakeItemToSlot { bag_slot, bag_item_slot_idx, inv_slot_idx } => {
                    let msg = ClientMsg::BagTakeItemToSlot { bag_slot, bag_item_slot_idx, inv_slot_idx };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bag take item to slot serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bag take item to slot send failed: {e}"))?;
                }
                GameNetCommand::BankDeposit { inv_slot_idx, item_id, quantity } => {
                    let msg = ClientMsg::BankDeposit {
                        inv_slot_idx,
                        item_id,
                        quantity: i64::from(quantity),
                    };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bank deposit serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bank deposit send failed: {e}"))?;
                }
                GameNetCommand::BankWithdraw { tab_idx, slot_idx, item_id, quantity } => {
                    let msg = ClientMsg::BankWithdraw {
                        tab_idx,
                        slot_idx,
                        item_id,
                        quantity: i64::from(quantity),
                    };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bank withdraw serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bank withdraw send failed: {e}"))?;
                }
                GameNetCommand::BankDepositAll => {
                    let msg = ClientMsg::BankDepositAll;
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bank deposit all serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bank deposit all send failed: {e}"))?;
                }
                GameNetCommand::BankClose => {
                    // Client-side only: no message to server needed.
                }
                GameNetCommand::BankCreateTab { display_name } => {
                    let msg = ClientMsg::BankCreateTab {
                        display_name,
                        tag_filters: vec![],
                    };
                    let json = serde_json::to_string(&msg)
                        .map_err(|e| format!("game ws bank create tab serialize failed: {e}"))?;
                    socket
                        .send(Message::Text(json))
                        .map_err(|e| format!("game ws bank create tab send failed: {e}"))?;
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
                                pos_x: p.pos_x,
                                pos_z: p.pos_z,
                                last_input_seq: p.last_input_seq,
                                tile: p.tile,
                                next_tile: p.next_tile,
                                goal: p.goal,
                                moving: p.moving,
                                move_progress: p.move_progress,
                                action: p.action.clone(),
                            })
                            .collect();

                        let local_player = player_id
                            .and_then(|pid| snap.players.iter().find(|p| p.player_id == pid));

                        let server_pos = local_player.map(|p| [p.pos_x, p.pos_z]);
                        let server_tile = local_player.map(|p| p.tile);
                        let server_next_tile = local_player.and_then(|p| p.next_tile);
                        let server_goal = local_player.and_then(|p| p.goal);
                        let server_moving = local_player.map(|p| p.moving).unwrap_or(false);
                        let server_move_progress = local_player.map(|p| p.move_progress).unwrap_or(0.0);
                        let server_action = local_player.and_then(|p| p.action.clone());

                        let _ = tx.send(GameNetEvent::Snapshot {
                            server_tick: snap.server_tick,
                            players,
                            harvest_nodes: snap.harvest_nodes,
                            server_pos,
                            server_tile,
                            server_next_tile,
                            server_goal,
                            server_moving,
                            server_move_progress,
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
                    Ok(ServerMsg::HarvestResult(result)) => {
                        let _ = tx.send(GameNetEvent::HarvestResult(result));
                    }
                    Ok(ServerMsg::HarvestNodeEvent(event)) => {
                        let _ = tx.send(GameNetEvent::HarvestNodeEvent(event));
                    }
                    Ok(ServerMsg::InventorySnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::InventorySnapshot(snapshot));
                    }
                    Ok(ServerMsg::InventoryDelta(delta)) => {
                        let _ = tx.send(GameNetEvent::InventoryDelta(delta));
                    }
                    Ok(ServerMsg::GroundItemsSnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::GroundItemsSnapshot(snapshot));
                    }
                    Ok(ServerMsg::GroundItemEvent(event)) => {
                        let _ = tx.send(GameNetEvent::GroundItemEvent(event));
                    }
                    Ok(ServerMsg::SkillSnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::SkillSnapshot(snapshot));
                    }
                    Ok(ServerMsg::SkillDelta(delta)) => {
                        let _ = tx.send(GameNetEvent::SkillDelta(delta));
                    }
                    Ok(ServerMsg::BagSlotsSnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::BagSlotsSnapshot(snapshot));
                    }
                    Ok(ServerMsg::BagSlotChanged(changed)) => {
                        let _ = tx.send(GameNetEvent::BagSlotChanged(changed));
                    }
                    Ok(ServerMsg::EquipmentSnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::EquipmentSnapshot(snapshot));
                    }
                    Ok(ServerMsg::BankSnapshot(snapshot)) => {
                        let _ = tx.send(GameNetEvent::BankSnapshot(snapshot));
                    }
                    Ok(ServerMsg::BankTabChanged(tab)) => {
                        let _ = tx.send(GameNetEvent::BankTabChanged(tab));
                    }
                    Ok(ServerMsg::PathConfirmed { goal, tiles }) => {
                        let _ = tx.send(GameNetEvent::PathConfirmed { goal, tiles });
                    }
                    Ok(ServerMsg::Error { message }) => {
                        let _ = tx.send(GameNetEvent::ServerNotice { player_id: None, message });
                    }
                    Ok(ServerMsg::Notice { player_id, message }) => {
                        let _ = tx.send(GameNetEvent::ServerNotice {
                            player_id: Some(player_id),
                            message,
                        });
                    }
                    Ok(ServerMsg::WorldCollision { blocked }) => {
                        let _ = tx.send(GameNetEvent::WorldCollision(blocked));
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
    mut server_blocked: ResMut<ServerBlockedTiles>,
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
            GameNetEvent::WorldCollision(blocked) => {
                let set: std::collections::HashSet<_> = blocked.into_iter().collect();
                info!("game net world collision: {} blocked tiles", set.len());
                server_blocked.0 = Some(set);
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
                server_pos,
                server_tile,
                server_next_tile,
                server_goal,
                server_moving,
                server_move_progress,
                server_action,
            } => {
                status.server_tick = Some(server_tick);
                status.snapshot_players = players.len();
                status.latest_players = players;
                status.harvest_nodes = harvest_nodes;
                status.server_pos = server_pos;
                status.server_moving = server_moving;
                status.server_move_progress = server_move_progress;
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

                    if action == InteractionAction::Harvest {
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
                            status.action_event_in_flight = true;
                            if let InteractionTarget::Tile(tile) = target {
                                status.action_marker_target = Some(tile);
                            }
                        }
                        // All terminal states mean the same thing to the client now:
                        // the server-driven harvest loop has ended, so stop animating.
                        ActionState::Complete | ActionState::Cancelled | ActionState::Rejected => {
                            status.action_event_in_flight = false;
                            status.server_action = None;
                            status.action_marker_target = None;
                        }
                    }
                }
            }
            GameNetEvent::HarvestResult(result) => {
                match result.item_id.as_ref() {
                    Some(item_id) => info!(
                        "game net harvest result success={} node={} item={} quantity={} inventory_quantity={:?} charges_remaining={}",
                        result.success,
                        result.node_id,
                        item_id,
                        result.quantity,
                        result.inventory_quantity,
                        result.charges_remaining
                    ),
                    None => info!(
                        "game net harvest result success={} node={} charges_remaining={}",
                        result.success,
                        result.node_id,
                        result.charges_remaining
                    ),
                }
                // Right-side "+N item" drop for a successful harvest that yielded loot.
                if result.success {
                    if let Some(item_id) = result.item_id.as_ref() {
                        status.feedback_drops.push(super::status::FeedbackDrop::Item {
                            item_id: item_id.clone(),
                            quantity: result.quantity,
                        });
                    }
                }
            }
            GameNetEvent::HarvestNodeEvent(event) => {
                info!(
                    "game net harvest node event kind={:?} node={} tile={},{} charges_remaining={}",
                    event.kind,
                    event.node_id,
                    event.tile.x,
                    event.tile.y,
                    event.charges_remaining
                );

                let depleted = matches!(
                    event.kind,
                    super::protocol::HarvestNodeEventKind::Depleted
                );

                let snapshot = super::protocol::HarvestNodeSnapshot {
                    node_id: event.node_id.clone(),
                    node_def_id: event.node_def_id.clone(),
                    display_name: event.display_name.clone(),
                    tile: event.tile,
                    charges_remaining: event.charges_remaining,
                    max_charges: event.max_charges,
                    depleted,
                    depleted_until_tick: event.depleted_until_tick,
                    available_model: event.available_model.clone(),
                    depleted_model: event.depleted_model.clone(),
                };

                if let Some(existing) = status
                    .harvest_nodes
                    .iter_mut()
                    .find(|node| node.node_id == event.node_id)
                {
                    *existing = snapshot;
                } else {
                    status.harvest_nodes.push(snapshot);
                }
            }
            GameNetEvent::InventorySnapshot(snapshot) => {
                info!(
                    "game net inventory snapshot character_id={} slots_total={} items={}",
                    snapshot.character_id,
                    snapshot.slots_total,
                    snapshot.items.len()
                );
                status.inventory_slots_total = snapshot.slots_total;
                status.inventory_items = snapshot.items;
                status.inventory_dirty = true;
            }
            GameNetEvent::InventoryDelta(delta) => {
                info!(
                    "game net inventory delta character_id={} slot_idx={:?} item_id={} delta={} new_quantity={}",
                    delta.character_id,
                    delta.slot_idx,
                    delta.item_id,
                    delta.quantity_delta,
                    delta.new_quantity
                );

                if status.character_id != Some(delta.character_id) {
                    continue;
                }

                if let Some(slot_idx) = delta.slot_idx {
                    if delta.new_quantity <= 0 {
                        status.inventory_items.retain(|item| item.slot_idx != slot_idx);
                    } else if let Some(item) = status
                        .inventory_items
                        .iter_mut()
                        .find(|item| item.slot_idx == slot_idx)
                    {
                        item.item_id = delta.item_id.clone();
                        item.quantity = delta.new_quantity;
                    } else {
                        status.inventory_items.push(super::protocol::InventoryItemSnapshot {
                            slot_idx,
                            item_id: delta.item_id.clone(),
                            quantity: delta.new_quantity,
                        });
                    }
                    status.inventory_items.sort_by_key(|item| item.slot_idx);
                    status.inventory_dirty = true;
                }
            }
            GameNetEvent::GroundItemsSnapshot(snapshot) => {
                info!("game net ground items snapshot items={}", snapshot.items.len());
                status.ground_items = snapshot.items;
                status.ground_items_dirty = true;
            }
            GameNetEvent::GroundItemEvent(event) => {
                match event.kind {
                    GroundItemEventKind::Spawned => {
                        if let Some(item) = event.item {
                            info!(
                                "game net ground item spawned id={} item={} quantity={} tile={},{}",
                                item.ground_item_id,
                                item.item_id,
                                item.quantity,
                                item.tile.x,
                                item.tile.y
                            );
                            if let Some(existing) = status
                                .ground_items
                                .iter_mut()
                                .find(|existing| existing.ground_item_id == item.ground_item_id)
                            {
                                *existing = item;
                            } else {
                                status.ground_items.push(item);
                            }
                        }
                    }
                    GroundItemEventKind::PickedUp | GroundItemEventKind::Despawned => {
                        info!(
                            "game net ground item removed kind={:?} id={}",
                            event.kind, event.ground_item_id
                        );
                        status
                            .ground_items
                            .retain(|item| item.ground_item_id != event.ground_item_id);
                    }
                }
                status.ground_items_dirty = true;
            }
            GameNetEvent::SkillSnapshot(snapshot) => {
                info!(
                    "game net skill snapshot character_id={} skills={}",
                    snapshot.character_id,
                    snapshot.skills.len()
                );

                if status.character_id == Some(snapshot.character_id) {
                    status.skill_entries = snapshot.skills;
                    status.skills_dirty = true;
                }
            }
            GameNetEvent::BagSlotsSnapshot(snapshot) => {
                info!(
                    "game net bag slots snapshot character_id={} slots={}",
                    snapshot.character_id,
                    snapshot.slots.len()
                );
                if status.character_id == Some(snapshot.character_id) {
                    status.bag_slots = snapshot.slots;
                    status.bag_slots_dirty = true;
                }
            }
            GameNetEvent::EquipmentSnapshot(snapshot) => {
                info!(
                    "game net equipment snapshot character_id={} slots={}",
                    snapshot.character_id,
                    snapshot.slots.len()
                );
                if status.character_id == Some(snapshot.character_id) {
                    status.equipment = snapshot.slots;
                    status.equipment_dirty = true;
                }
            }
            GameNetEvent::BagSlotChanged(changed) => {
                info!(
                    "game net bag slot changed character_id={} bag_slot={}",
                    changed.character_id, changed.slot.bag_slot
                );
                if status.character_id == Some(changed.character_id) {
                    let bag_slot = changed.slot.bag_slot as usize;
                    if let Some(existing) = status.bag_slots.iter_mut().find(|s| s.bag_slot == changed.slot.bag_slot) {
                        *existing = changed.slot;
                    } else if bag_slot < 2 {
                        status.bag_slots.push(changed.slot);
                        status.bag_slots.sort_by_key(|s| s.bag_slot);
                    }
                    status.bag_slots_dirty = true;
                }
            }
            GameNetEvent::SkillDelta(delta) => {
                info!(
                    "game net skill delta character_id={} skill={} xp_delta={} new_xp={} new_level={}",
                    delta.character_id,
                    delta.skill_id,
                    delta.xp_delta,
                    delta.new_xp,
                    delta.new_level
                );

                if status.character_id != Some(delta.character_id) {
                    continue;
                }

                if let Some(skill) = status
                    .skill_entries
                    .iter_mut()
                    .find(|skill| skill.skill_id == delta.skill_id)
                {
                    skill.display_name = delta.display_name.clone();
                    skill.xp = delta.new_xp;
                    skill.level = delta.new_level;
                    skill.xp_for_next_level = delta.xp_for_next_level;
                } else {
                    status.skill_entries.push(super::protocol::SkillSnapshotEntry {
                        skill_id: delta.skill_id.clone(),
                        display_name: delta.display_name.clone(),
                        xp: delta.new_xp,
                        level: delta.new_level,
                        xp_for_next_level: delta.xp_for_next_level,
                    });
                }

                status.skill_entries.sort_by(|a, b| a.skill_id.cmp(&b.skill_id));
                status.skills_dirty = true;

                if delta.xp_delta > 0 {
                    status.feedback_drops.push(super::status::FeedbackDrop::Xp {
                        skill_display: delta.display_name,
                        amount: delta.xp_delta,
                    });
                }
            }
            GameNetEvent::BankSnapshot(snapshot) => {
                info!(
                    "game net bank snapshot character_id={} tabs={}",
                    snapshot.character_id,
                    snapshot.tabs.len()
                );
                if status.character_id == Some(snapshot.character_id) {
                    status.bank_tabs = snapshot.tabs;
                    status.bank_dirty = true;
                    status.bank_open = true;
                }
            }
            GameNetEvent::BankTabChanged(tab) => {
                info!(
                    "game net bank tab changed character_id={} tab_idx={} items={}",
                    tab.character_id,
                    tab.tab_idx,
                    tab.items.len()
                );
                if status.character_id == Some(tab.character_id) {
                    if let Some(existing) = status.bank_tabs.iter_mut().find(|t| t.tab_idx == tab.tab_idx) {
                        *existing = tab;
                    } else {
                        status.bank_tabs.push(tab);
                        status.bank_tabs.sort_by_key(|t| t.tab_idx);
                    }
                    status.bank_dirty = true;
                }
            }
            GameNetEvent::PathConfirmed { goal, tiles } => {
                debug!(
                    "game net path confirmed goal={},{} tiles={}",
                    goal.x,
                    goal.y,
                    tiles.len()
                );
                status.pending_server_path = Some((goal, tiles));
            }
            GameNetEvent::ServerNotice { player_id, message } => {
                // Player-facing server message (wield-gate/equip/bank rejection,
                // inventory full, etc.): surface it as a right-side red drop.
                // `None` = targeted to our connection; `Some` = broadcast for a
                // specific player, so only that player shows it (no leak).
                let for_me = player_id.map_or(true, |pid| status.player_id == Some(pid));
                if for_me {
                    status.last_error = Some(message.clone());
                    status
                        .feedback_drops
                        .push(super::status::FeedbackDrop::Message { text: message.clone() });
                }
                warn!("game net notice: {}", message);
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
                status.ground_items.clear();
                status.ground_items_dirty = true;
                status.server_next_tile = None;
                status.server_goal = None;
                status.server_moving = false;
                status.server_action = None;
                status.inventory_slots_total = 20;
                status.inventory_items.clear();
                status.inventory_dirty = true;
                status.bag_slots.clear();
                status.bag_slots_dirty = true;
                status.skill_entries.clear();
                status.skills_dirty = true;
                status.feedback_drops.clear();
                status.action_marker_target = None;
                status.remote_player_count = 0;
                status.pending_server_path = None;
                status.pending_path_confirmations = 0;
                warn!("game net disconnected");
            }
        }
    }
}

pub fn send_walk_intents_to_server_runtime(
    mut intents: MessageReader<IntentMsg>,
    game_net: Res<GameNetRuntime>,
    mut status: ResMut<GameNetStatus>,
    mut pending_pickup: ResMut<PendingGroundItemPickup>,
    mut pending_bank_open: ResMut<PendingBankOpen>,
    grid_pos_q: Query<&GridPos>,
    ground_item_q: Query<(&super::ground_items::ServerGroundItemVisual, &GridPos)>,
    mut player_q: Query<(Entity, &mut TilePath), With<Player>>,
    mut commands: Commands,
) {
    for ev in intents.read() {
        match ev.intent.verb {
            Verb::WalkHere => {
                let Target::Tile(tile) = ev.intent.target else {
                    continue;
                };

                pending_pickup.request = None;
                pending_bank_open.booth_tile = None;

                if !send_move_to_server(&game_net, tile) {
                    warn!("game net move target dropped; websocket is not ready");
                } else {
                    // Clear path + remove StepTo + increment counter atomically in
                    // this system so the reconciler never sees an inconsistent state
                    // (cleared path with counter still 0). If reconciler runs before
                    // this system it sees the old full path; if after, it sees the
                    // empty path and counter > 0, which suppresses heuristics.
                    if let Ok((entity, mut path)) = player_q.single_mut() {
                        path.tiles.clear();
                        commands.entity(entity).remove::<StepTo>();
                    }
                    status.pending_path_confirmations += 1;
                }
            }
            Verb::Harvest => {
                let Some(tile) = intent_target_tile(ev.intent.target, &grid_pos_q) else {
                    warn!("game net chopdown target dropped; target tile could not be resolved");
                    continue;
                };

                pending_pickup.request = None;
                pending_bank_open.booth_tile = None;

                if !send_interaction_to_server(
                    &game_net,
                    InteractionAction::Harvest,
                    InteractionTarget::Tile(tile),
                ) {
                    warn!("game net chopdown target dropped; websocket is not ready");
                }
            }
            Verb::Take => {
                let Target::Entity(entity) = ev.intent.target else {
                    warn!("game net take target dropped; target was not an entity");
                    continue;
                };

                let Ok((ground_item, grid_pos)) = ground_item_q.get(entity) else {
                    warn!("game net take target dropped; entity was not a ground item");
                    continue;
                };

                pending_pickup.request = Some(PendingGroundItemPickupRequest {
                    ground_item_id: ground_item.ground_item_id,
                    tile: grid_pos.0,
                });

                if !send_move_to_server(&game_net, grid_pos.0) {
                    warn!(
                        "game net take move target dropped; websocket is not ready ground_item_id={}",
                        ground_item.ground_item_id
                    );
                }
            }
            Verb::UseBank => {
                let Some(booth_tile) = intent_target_tile(ev.intent.target, &grid_pos_q) else {
                    warn!("game net use bank target dropped; target tile could not be resolved");
                    continue;
                };
                // Store the booth tile. process_pending_bank_open will fire the
                // actual UseBank interaction once the server-reconciled position is
                // within 1 Chebyshev tile of the booth.
                //
                // We deliberately do NOT send MoveTo here. Sending MoveTo switches
                // the server into tile-walk mode, which makes server_pos tile-aligned
                // and causes the client reconciler to snap to those tile positions.
                // Since UseBank is only triggered via E-key proximity (player already
                // adjacent) or a right-click from nearby, the server will be close
                // enough within one tick. Let the WASD-continuous flow stay clean.
                pending_bank_open.booth_tile = Some(booth_tile);
            }
            Verb::TalkTo | Verb::Examine => {}
        }
    }
}

pub fn process_pending_ground_item_pickups(
    game_net: Res<GameNetRuntime>,
    status: Res<GameNetStatus>,
    mut pending_pickup: ResMut<PendingGroundItemPickup>,
) {
    let Some(request) = pending_pickup.request.clone() else {
        return;
    };

    let Some(server_tile) = status.server_tile else {
        return;
    };

    let still_exists = status
        .ground_items
        .iter()
        .any(|item| item.ground_item_id == request.ground_item_id);

    if !still_exists {
        pending_pickup.request = None;
        return;
    }

    if manhattan(server_tile, request.tile) > 1 {
        return;
    }

    if send_pickup_ground_item_to_server(&game_net, request.ground_item_id) {
        pending_pickup.request = None;
    } else {
        warn!(
            "game net pending pickup dropped; websocket is not ready ground_item_id={}",
            request.ground_item_id
        );
    }
}

/// Fires the UseBank interaction once the server-reconciled player position is
/// within 1 Chebyshev tile of the pending booth.
pub fn process_pending_bank_open(
    game_net: Res<GameNetRuntime>,
    status: Res<GameNetStatus>,
    mut pending: ResMut<PendingBankOpen>,
) {
    let Some(booth_tile) = pending.booth_tile else {
        return;
    };

    let Some(server_tile) = status.server_tile else {
        return;
    };

    let dx = (server_tile.x - booth_tile.x).abs();
    let dy = (server_tile.y - booth_tile.y).abs();
    if dx.max(dy) > 1 {
        return;
    }

    if send_interaction_to_server(
        &game_net,
        InteractionAction::UseBank,
        InteractionTarget::Tile(booth_tile),
    ) {
        pending.booth_tile = None;
    } else {
        warn!("game net pending bank open dropped; websocket is not ready");
    }
}

/// Sends `ClientMsg::MoveDir` to the server whenever the player's movement
/// direction changes (key pressed or released).  Fires immediately on change —
/// no polling timer — so the server always has the current direction within one
/// frame of input.
///
/// Uses `[f32; 2]` stored in a `Local` to track the last sent direction and
/// avoid spamming identical messages every frame.
pub fn send_wasd_movement_to_server(
    keyboard: Res<ButtonInput<KeyCode>>,
    game_net: Res<GameNetRuntime>,
    mut status: ResMut<GameNetStatus>,
    mut last_sent: Local<[f32; 2]>,
) {
    // Build float direction — same axes as the client movement system.
    // Vec2 convention: x = world-X, y = world-Z (LogicalPos2d).
    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
    if keyboard.pressed(KeyCode::KeyW) { dy -= 1.0; }
    if keyboard.pressed(KeyCode::KeyS) { dy += 1.0; }
    if keyboard.pressed(KeyCode::KeyA) { dx -= 1.0; }
    if keyboard.pressed(KeyCode::KeyD) { dx += 1.0; }

    // Normalise so diagonal doesn't move faster.
    let len = (dx * dx + dy * dy).sqrt();
    let (dx, dy) = if len > 1e-6 {
        (dx / len, dy / len)
    } else {
        (0.0, 0.0)
    };

    // Only send when direction actually changes — avoids flooding the websocket.
    if (dx - last_sent[0]).abs() < 1e-6 && (dy - last_sent[1]).abs() < 1e-6 {
        return;
    }

    let guard = game_net.command_tx.lock().unwrap();
    if let Some(tx) = guard.as_ref() {
        let next = status.last_sent_input_seq.wrapping_add(1);
        if tx.send(GameNetCommand::MoveDir { dx, dy, seq: next }).is_ok() {
            status.last_sent_input_seq = next;
            *last_sent = [dx, dy];
        }
    }
}

fn intent_target_tile(target: Target, grid_pos_q: &Query<&GridPos>) -> Option<TilePos> {
    match target {
        Target::Tile(tile) => Some(tile),
        Target::Entity(entity) => grid_pos_q.get(entity).ok().map(|gp| gp.0),
    }
}

fn manhattan(a: TilePos, b: TilePos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
