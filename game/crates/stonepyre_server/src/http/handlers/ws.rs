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
        protocol::{
            ActionState,
            ClientMsg,
            GroundItemEvent,
            GroundItemEventKind,
            GroundItemsSnapshot,
            InteractionAction,
            InteractionTarget,
            ServerMsg,
        },
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

                        let ground_items = {
                            let sim = state.game.sim.read().await;
                            sim.ground_item_snapshots()
                        };
                        let _ = out_tx.send(ServerMsg::GroundItemsSnapshot(GroundItemsSnapshot {
                            items: ground_items,
                        }));

                        match crate::game::sim::skills::load_character_skills_snapshot(
                            &state.db,
                            requested_character_id,
                        )
                        .await
                        {
                            Ok(snapshot) => {
                                let _ = out_tx.send(ServerMsg::SkillSnapshot(snapshot));
                            }
                            Err(e) => {
                                warn!(
                                    "game ws skill snapshot failed account_id={} character_id={} error={:?}",
                                    ctx.account_id, requested_character_id, e
                                );
                                let _ = out_tx.send(ServerMsg::Error {
                                    message: "failed to load skills".to_string(),
                                });
                            }
                        }

                        match crate::game::sim::inventory::load_bag_slots_snapshot(
                            &state.db,
                            requested_character_id,
                        )
                        .await
                        {
                            Ok(snapshot) => {
                                let _ = out_tx.send(ServerMsg::BagSlotsSnapshot(snapshot));
                            }
                            Err(e) => {
                                warn!(
                                    "game ws bag slots snapshot failed account_id={} character_id={} error={:?}",
                                    ctx.account_id, requested_character_id, e
                                );
                                let _ = out_tx.send(ServerMsg::Error {
                                    message: "failed to load bag slots".to_string(),
                                });
                            }
                        }

                        match crate::game::sim::equipment::load_character_equipment(
                            &state.db,
                            requested_character_id,
                        )
                        .await
                        {
                            Ok(snapshot) => {
                                let _ = out_tx.send(ServerMsg::EquipmentSnapshot(snapshot));
                            }
                            Err(e) => {
                                warn!(
                                    "game ws equipment snapshot failed account_id={} character_id={} error={:?}",
                                    ctx.account_id, requested_character_id, e
                                );
                                let _ = out_tx.send(ServerMsg::Error {
                                    message: "failed to load equipment".to_string(),
                                });
                            }
                        }
                    }
                    ClientMsg::MoveDir { dx, dy } => {
                        if let Some(pid) = player_id {
                            // Normalise on the server as a safety measure — reject
                            // any inflated magnitude the client might send.
                            let len = (dx * dx + dy * dy).sqrt();
                            let dir = if len > 1e-6 {
                                [dx / len, dy / len]
                            } else {
                                [0.0, 0.0]
                            };
                            let cancelled = {
                                let mut sim = state.game.sim.write().await;
                                sim.set_move_dir(pid, dir)
                            };
                            if let Some(event) = cancelled {
                                let _ = out_tx.send(event);
                            }
                        }
                    }
                    ClientMsg::MoveTo { tile } => {
                        if let Some(pid) = player_id {
                            let (cancelled_event, path_confirmed) = {
                                let mut sim = state.game.sim.write().await;
                                let cancelled = sim.set_move_target(pid, tile);
                                let path_msg = sim
                                    .player_path_and_goal(pid)
                                    .map(|(goal, tiles)| ServerMsg::PathConfirmed { goal, tiles });
                                (cancelled, path_msg)
                            };

                            if let Some(event) = cancelled_event {
                                let _ = out_tx.send(event);
                            }
                            if let Some(msg) = path_confirmed {
                                let _ = out_tx.send(msg);
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

                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before sending interactions".to_string(),
                            });
                            continue;
                        };

                        handle_interaction(&state, pid, cid, action, target, &out_tx).await;
                    }
                    ClientMsg::DropItem {
                        slot_idx,
                        item_id,
                        quantity,
                    } => {
                        let Some(pid) = player_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before dropping items".to_string(),
                            });
                            continue;
                        };

                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before dropping items".to_string(),
                            });
                            continue;
                        };

                        handle_drop_item(&state, pid, cid, slot_idx, item_id, quantity, &out_tx).await;
                    }
                    ClientMsg::PickupGroundItem { ground_item_id } => {
                        let Some(pid) = player_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before picking up items".to_string(),
                            });
                            continue;
                        };

                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before picking up items".to_string(),
                            });
                            continue;
                        };

                        handle_pickup_ground_item(&state, pid, cid, ground_item_id, &out_tx).await;
                    }
                    ClientMsg::EquipBag { inventory_slot_idx, bag_slot } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before equipping bags".to_string(),
                            });
                            continue;
                        };
                        handle_equip_bag(&state, cid, inventory_slot_idx, bag_slot, &out_tx).await;
                    }
                    ClientMsg::UnequipBag { bag_slot } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before unequipping bags".to_string(),
                            });
                            continue;
                        };
                        handle_unequip_bag(&state, cid, bag_slot, &out_tx).await;
                    }
                    ClientMsg::EquipItem { inventory_slot_idx, item_id } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before equipping items".to_string(),
                            });
                            continue;
                        };
                        handle_equip_item(&state, cid, inventory_slot_idx, &item_id, &out_tx).await;
                    }
                    ClientMsg::UnequipItem { slot } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before unequipping items".to_string(),
                            });
                            continue;
                        };
                        handle_unequip_item(&state, cid, &slot, &out_tx).await;
                    }
                    ClientMsg::BagPutItem { bag_slot, inventory_slot_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before using bags".to_string(),
                            });
                            continue;
                        };
                        handle_bag_put_item(&state, cid, bag_slot, inventory_slot_idx, &out_tx).await;
                    }
                    ClientMsg::BagTakeItem { bag_slot, bag_item_slot_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before using bags".to_string(),
                            });
                            continue;
                        };
                        handle_bag_take_item(&state, cid, bag_slot, bag_item_slot_idx, &out_tx).await;
                    }
                    ClientMsg::SwapInvSlots { from_slot, to_slot } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before rearranging inventory".to_string(),
                            });
                            continue;
                        };
                        handle_swap_inv_slots(&state, cid, from_slot, to_slot, &out_tx).await;
                    }
                    ClientMsg::BagMoveItem { from_bag_slot, from_item_slot, to_bag_slot } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before moving bag items".to_string(),
                            });
                            continue;
                        };
                        handle_bag_move_item(&state, cid, from_bag_slot, from_item_slot, to_bag_slot, &out_tx).await;
                    }
                    ClientMsg::BagPutItemToSlot { bag_slot, inventory_slot_idx, bag_item_slot_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before using bags".to_string(),
                            });
                            continue;
                        };
                        handle_bag_put_item_to_slot(&state, cid, bag_slot, inventory_slot_idx, bag_item_slot_idx, &out_tx).await;
                    }
                    ClientMsg::BagTakeItemToSlot { bag_slot, bag_item_slot_idx, inv_slot_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error {
                                message: "join the world before using bags".to_string(),
                            });
                            continue;
                        };
                        handle_bag_take_item_to_slot(&state, cid, bag_slot, bag_item_slot_idx, inv_slot_idx, &out_tx).await;
                    }

                    // ------------------------------------------------------------------
                    // Bank commands
                    // ------------------------------------------------------------------

                    ClientMsg::BankDeposit { inv_slot_idx, item_id, quantity } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_deposit(&state, cid, inv_slot_idx, item_id, quantity, &out_tx).await;
                    }
                    ClientMsg::BankWithdraw { tab_idx, slot_idx, item_id, quantity } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_withdraw(&state, cid, tab_idx, slot_idx, item_id, quantity, &out_tx).await;
                    }
                    ClientMsg::BankDepositAll => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_deposit_all(&state, cid, &out_tx).await;
                    }
                    ClientMsg::BankCreateTab { display_name, tag_filters } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_create_tab(&state, cid, display_name, tag_filters, &out_tx).await;
                    }
                    ClientMsg::BankUpdateTab { tab_idx, display_name, tag_filters } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_update_tab(&state, cid, tab_idx, display_name, tag_filters, &out_tx).await;
                    }
                    ClientMsg::BankDeleteTab { tab_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_delete_tab(&state, cid, tab_idx, &out_tx).await;
                    }
                    ClientMsg::BankMoveItem { from_tab_idx, slot_idx, item_id, to_tab_idx } => {
                        let Some(cid) = character_id else {
                            let _ = out_tx.send(ServerMsg::Error { message: "join the world first".to_string() });
                            continue;
                        };
                        handle_bank_move_item(&state, cid, from_tab_idx, slot_idx, item_id, to_tab_idx, &out_tx).await;
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
    character_id: Uuid,
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
        (InteractionAction::UseBank, InteractionTarget::Tile(booth_tile)) => {
            // Proximity check: the player must be adjacent (Chebyshev ≤ 1) to the booth.
            let player_tile = {
                let sim = state.game.sim.read().await;
                sim.player_tile(player_id)
            };

            let in_range = player_tile
                .map(|pt| {
                    let dx = (pt.x - booth_tile.x).abs();
                    let dy = (pt.y - booth_tile.y).abs();
                    dx.max(dy) <= 1
                })
                .unwrap_or(false);

            if !in_range {
                let _ = out_tx.send(ServerMsg::InteractionAck {
                    accepted: false,
                    action,
                    target,
                    message: "too far from the bank".to_string(),
                });
                return;
            }

            let _ = out_tx.send(ServerMsg::InteractionAck {
                accepted: true,
                action,
                target,
                message: "bank opened".to_string(),
            });
            match crate::game::sim::bank::load_bank_snapshot(&state.db, character_id).await {
                Ok(snapshot) => {
                    let _ = out_tx.send(ServerMsg::BankSnapshot(snapshot));
                }
                Err(e) => {
                    warn!("bank snapshot failed character_id={} error={:?}", character_id, e);
                    let _ = out_tx.send(ServerMsg::Error { message: "failed to load bank".to_string() });
                }
            }
        }
        (InteractionAction::ChopDown, InteractionTarget::Tile(tile)) => {
            let skill_requirement = {
                let sim = state.game.sim.read().await;
                sim.world.harvest_node_def_at(tile).map(|def| {
                    (
                        def.skill.id().to_string(),
                        def.skill.display_name().to_string(),
                    )
                })
            };

            let skill_level = if let Some((skill_id, skill_display_name)) = skill_requirement {
                match crate::game::sim::skills::load_character_skill_progress(
                    &state.db,
                    character_id,
                    &skill_id,
                )
                .await
                {
                    Ok(progress) => progress.level,
                    Err(e) => {
                        warn!(
                            "game ws skill level check failed character_id={} skill_id={} error={:?}",
                            character_id, skill_id, e
                        );

                        let message = format!("failed to load {} level", skill_display_name);

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

                        return;
                    }
                }
            } else {
                1
            };

            let validation = {
                let mut sim = state.game.sim.write().await;
                sim.queue_chop_down(player_id, tile, skill_level)
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

async fn handle_drop_item(
    state: &AppState,
    player_id: Uuid,
    character_id: Uuid,
    slot_idx: usize,
    item_id: String,
    quantity: u32,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::remove_character_item_from_slot(
        &state.db,
        character_id,
        slot_idx,
        &item_id,
        quantity,
    )
    .await
    {
        Ok(result) => {
            info!(
                "game ws dropped item character_id={} slot_idx={} item_id={} quantity={} new_quantity={}",
                character_id, result.slot_idx, result.item_id, result.quantity_removed, result.new_quantity
            );

            let event = {
                let mut sim = state.game.sim.write().await;
                sim.spawn_ground_item_for_player(
                    player_id,
                    result.item_id.clone(),
                    result.quantity_removed,
                    Some(character_id),
                )
            };

            match event {
                Ok(event) => {
                    state.game.hub.broadcast(ServerMsg::GroundItemEvent(event));
                    send_inventory_snapshot(state, character_id, out_tx).await;
                }
                Err(message) => {
                    warn!(
                        "ground item spawn failed after inventory removal character_id={} item_id={} quantity={} error={}",
                        character_id, result.item_id, result.quantity_removed, message
                    );
                    let _ = crate::game::sim::inventory::grant_character_item(
                        &state.db,
                        character_id,
                        &result.item_id,
                        result.quantity_removed,
                    )
                    .await;
                    let _ = out_tx.send(ServerMsg::Error { message });
                }
            }
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::InvalidQuantity) => {
            let _ = out_tx.send(ServerMsg::Error {
                message: "drop quantity must be greater than zero".to_string(),
            });
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::InvalidSlot { slot_idx }) => {
            let _ = out_tx.send(ServerMsg::Error {
                message: format!("invalid inventory slot {}", slot_idx),
            });
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::SlotEmpty { slot_idx }) => {
            let _ = out_tx.send(ServerMsg::Error {
                message: format!("inventory slot {} is empty", slot_idx),
            });
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::SlotItemMismatch {
            slot_idx,
            expected_item_id,
            actual_item_id,
        }) => {
            let _ = out_tx.send(ServerMsg::Error {
                message: format!(
                    "inventory slot {} changed before drop: expected {}, found {}",
                    slot_idx, expected_item_id, actual_item_id
                ),
            });
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::InsufficientQuantity {
            item_id,
            requested,
            available,
        }) => {
            let _ = out_tx.send(ServerMsg::Error {
                message: format!(
                    "not enough {} to drop: requested {}, available {}",
                    item_id, requested, available
                ),
            });
        }
        Err(crate::game::sim::inventory::InventoryRemoveError::Db(e)) => {
            warn!(
                "inventory drop removal failed character_id={} slot_idx={} item_id={} quantity={} error={:?}",
                character_id, slot_idx, item_id, quantity, e
            );
            let _ = out_tx.send(ServerMsg::Error {
                message: "failed to drop item".to_string(),
            });
        }
    }
}

async fn handle_pickup_ground_item(
    state: &AppState,
    player_id: Uuid,
    character_id: Uuid,
    ground_item_id: Uuid,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    let ground_item = {
        let mut sim = state.game.sim.write().await;
        sim.take_ground_item_for_player(player_id, character_id, ground_item_id)
    };

    let ground_item = match ground_item {
        Ok(item) => item,
        Err(message) => {
            let _ = out_tx.send(ServerMsg::Error { message });
            return;
        }
    };

    match crate::game::sim::inventory::grant_character_item(
        &state.db,
        character_id,
        &ground_item.item_id,
        ground_item.quantity,
    )
    .await
    {
        Ok(_) => {
            let picked_up = GroundItemEvent {
                kind: GroundItemEventKind::PickedUp,
                item: None,
                ground_item_id,
            };
            state.game.hub.broadcast(ServerMsg::GroundItemEvent(picked_up));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(crate::game::sim::inventory::InventoryGrantError::InventoryFull { .. }) => {
            {
                let mut sim = state.game.sim.write().await;
                sim.restore_ground_item(ground_item);
            }
            let _ = out_tx.send(ServerMsg::Error {
                message: "Inventory full".to_string(),
            });
        }
        Err(crate::game::sim::inventory::InventoryGrantError::Db(e)) => {
            warn!(
                "ground item pickup inventory grant failed character_id={} ground_item_id={} error={:?}",
                character_id, ground_item_id, e
            );
            {
                let mut sim = state.game.sim.write().await;
                sim.restore_ground_item(ground_item);
            }
            let _ = out_tx.send(ServerMsg::Error {
                message: "failed to pick up item".to_string(),
            });
        }
    }
}

async fn send_inventory_snapshot(
    state: &AppState,
    character_id: Uuid,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::load_character_inventory_snapshot(&state.db, character_id).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::InventorySnapshot(snapshot));
        }
        Err(e) => {
            warn!(
                "inventory snapshot refresh failed character_id={} error={:?}",
                character_id, e
            );
            let _ = out_tx.send(ServerMsg::Error {
                message: "failed to refresh inventory".to_string(),
            });
        }
    }
}

async fn handle_equip_bag(
    state: &AppState,
    character_id: Uuid,
    inventory_slot_idx: usize,
    bag_slot: u8,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::equip_bag(&state.db, character_id, inventory_slot_idx, bag_slot).await {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!("equip bag failed character_id={} bag_slot={} error={}", character_id, bag_slot, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_unequip_bag(
    state: &AppState,
    character_id: Uuid,
    bag_slot: u8,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::unequip_bag(&state.db, character_id, bag_slot).await {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!("unequip bag failed character_id={} bag_slot={} error={}", character_id, bag_slot, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_equip_item(
    state: &AppState,
    character_id: Uuid,
    inventory_slot_idx: usize,
    item_id: &str,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::equipment::equip_item(&state.db, character_id, inventory_slot_idx, item_id).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::EquipmentSnapshot(snapshot));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = equip_error_message(e);
            warn!("equip item failed character_id={} slot_idx={} error={}", character_id, inventory_slot_idx, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_unequip_item(
    state: &AppState,
    character_id: Uuid,
    slot: &str,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::equipment::unequip_item(&state.db, character_id, slot).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::EquipmentSnapshot(snapshot));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = equip_error_message(e);
            warn!("unequip item failed character_id={} slot={} error={}", character_id, slot, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

fn equip_error_message(e: crate::game::sim::equipment::EquipError) -> String {
    use crate::game::sim::equipment::EquipError;
    match e {
        EquipError::SlotEmpty { .. } => "that inventory slot is empty".to_string(),
        EquipError::NotEquippable { .. } => "that item can't be equipped".to_string(),
        EquipError::InventoryFull => "your inventory is full".to_string(),
        EquipError::Db(_) => "failed to update equipment".to_string(),
    }
}

async fn handle_bag_put_item(
    state: &AppState,
    character_id: Uuid,
    bag_slot: u8,
    inventory_slot_idx: usize,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::bag_put_item(&state.db, character_id, bag_slot, inventory_slot_idx, None).await {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!("bag put item failed character_id={} bag_slot={} error={}", character_id, bag_slot, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_bag_take_item(
    state: &AppState,
    character_id: Uuid,
    bag_slot: u8,
    bag_item_slot_idx: usize,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::bag_take_item(&state.db, character_id, bag_slot, bag_item_slot_idx, None).await {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!("bag take item failed character_id={} bag_slot={} error={}", character_id, bag_slot, message);
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_bag_put_item_to_slot(
    state: &AppState,
    character_id: Uuid,
    bag_slot: u8,
    inventory_slot_idx: usize,
    bag_item_slot_idx: usize,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::bag_put_item(
        &state.db, character_id, bag_slot, inventory_slot_idx, Some(bag_item_slot_idx),
    )
    .await
    {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!(
                "bag put item to slot failed character_id={} bag_slot={} inv_slot={} bag_item_slot={} error={}",
                character_id, bag_slot, inventory_slot_idx, bag_item_slot_idx, message
            );
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_bag_take_item_to_slot(
    state: &AppState,
    character_id: Uuid,
    bag_slot: u8,
    bag_item_slot_idx: usize,
    inv_slot_idx: usize,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::bag_take_item(
        &state.db, character_id, bag_slot, bag_item_slot_idx, Some(inv_slot_idx),
    )
    .await
    {
        Ok(changed) => {
            let _ = out_tx.send(ServerMsg::BagSlotChanged(changed));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!(
                "bag take item to slot failed character_id={} bag_slot={} bag_item_slot={} inv_slot={} error={}",
                character_id, bag_slot, bag_item_slot_idx, inv_slot_idx, message
            );
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_swap_inv_slots(
    state: &AppState,
    character_id: Uuid,
    from_slot: usize,
    to_slot: usize,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::swap_inv_slots(&state.db, character_id, from_slot, to_slot).await {
        Ok(()) => {
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!(
                "swap inv slots failed character_id={} from={} to={} error={}",
                character_id, from_slot, to_slot, message
            );
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

async fn handle_bag_move_item(
    state: &AppState,
    character_id: Uuid,
    from_bag_slot: u8,
    from_item_slot: usize,
    to_bag_slot: u8,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::inventory::bag_move_item(
        &state.db,
        character_id,
        from_bag_slot,
        from_item_slot,
        to_bag_slot,
    )
    .await
    {
        Ok(changed) => {
            for c in changed {
                let _ = out_tx.send(ServerMsg::BagSlotChanged(c));
            }
        }
        Err(e) => {
            let message = bag_error_message(e);
            warn!(
                "bag move item failed character_id={} from_bag={} from_slot={} to_bag={} error={}",
                character_id, from_bag_slot, from_item_slot, to_bag_slot, message
            );
            let _ = out_tx.send(ServerMsg::Error { message });
        }
    }
}

// ---------------------------------------------------------------------------
// Bank handlers
// ---------------------------------------------------------------------------

async fn handle_bank_deposit(
    state: &AppState,
    character_id: Uuid,
    inv_slot_idx: usize,
    item_id: String,
    quantity: i64,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_deposit_item(&state.db, character_id, inv_slot_idx, &item_id, quantity).await {
        Ok(tab) => {
            let _ = out_tx.send(ServerMsg::BankTabChanged(tab));
            send_inventory_snapshot(state, character_id, out_tx).await;
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank deposit failed character_id={} slot={} item={} qty={} error={}", character_id, inv_slot_idx, item_id, quantity, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_withdraw(
    state: &AppState,
    character_id: Uuid,
    tab_idx: u8,
    slot_idx: usize,
    item_id: String,
    quantity: i64,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_withdraw_item(&state.db, character_id, tab_idx, slot_idx, &item_id, quantity).await {
        Ok((tab, inv)) => {
            let _ = out_tx.send(ServerMsg::BankTabChanged(tab));
            let _ = out_tx.send(ServerMsg::InventorySnapshot(inv));
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank withdraw failed character_id={} tab={} slot={} item={} qty={} error={}", character_id, tab_idx, slot_idx, item_id, quantity, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_deposit_all(
    state: &AppState,
    character_id: Uuid,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_deposit_all(&state.db, character_id).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::BankSnapshot(snapshot));
            send_inventory_snapshot(state, character_id, out_tx).await;
            // Also refresh bag snapshots since we emptied them.
            match crate::game::sim::inventory::load_bag_slots_snapshot(&state.db, character_id).await {
                Ok(bag_snapshot) => {
                    let _ = out_tx.send(ServerMsg::BagSlotsSnapshot(bag_snapshot));
                }
                Err(e) => {
                    warn!("bag snapshot refresh after deposit-all failed character_id={} error={:?}", character_id, e);
                }
            }
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank deposit all failed character_id={} error={}", character_id, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_create_tab(
    state: &AppState,
    character_id: Uuid,
    display_name: String,
    tag_filters: Vec<String>,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_create_tab(&state.db, character_id, &display_name, tag_filters).await {
        Ok(tab) => {
            let _ = out_tx.send(ServerMsg::BankTabChanged(tab));
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank create tab failed character_id={} name={} error={}", character_id, display_name, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_update_tab(
    state: &AppState,
    character_id: Uuid,
    tab_idx: u8,
    display_name: String,
    tag_filters: Vec<String>,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_update_tab(&state.db, character_id, tab_idx, &display_name, tag_filters).await {
        Ok(tab) => {
            let _ = out_tx.send(ServerMsg::BankTabChanged(tab));
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank update tab failed character_id={} tab={} error={}", character_id, tab_idx, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_delete_tab(
    state: &AppState,
    character_id: Uuid,
    tab_idx: u8,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_delete_tab(&state.db, character_id, tab_idx).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::BankSnapshot(snapshot));
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank delete tab failed character_id={} tab={} error={}", character_id, tab_idx, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

async fn handle_bank_move_item(
    state: &AppState,
    character_id: Uuid,
    from_tab_idx: u8,
    slot_idx: usize,
    item_id: String,
    to_tab_idx: u8,
    out_tx: &mpsc::UnboundedSender<ServerMsg>,
) {
    match crate::game::sim::bank::bank_move_item(&state.db, character_id, from_tab_idx, slot_idx, &item_id, to_tab_idx).await {
        Ok(snapshot) => {
            let _ = out_tx.send(ServerMsg::BankSnapshot(snapshot));
        }
        Err(e) => {
            let msg = bank_error_message(e);
            warn!("bank move item failed character_id={} from_tab={} slot={} item={} to_tab={} error={}", character_id, from_tab_idx, slot_idx, item_id, to_tab_idx, msg);
            let _ = out_tx.send(ServerMsg::Error { message: msg });
        }
    }
}

fn bank_error_message(e: crate::game::sim::bank::BankError) -> String {
    use crate::game::sim::bank::BankError;
    match e {
        BankError::TabNotFound { tab_idx } => format!("bank tab {} not found", tab_idx),
        BankError::TabLimitReached => "maximum bank tabs reached".to_string(),
        BankError::CannotModifyAllTab => "cannot modify the All tab".to_string(),
        BankError::FilterConflict { item_tag, existing_tab } => {
            format!("tag '{}' already used by tab {}", item_tag, existing_tab)
        }
        BankError::SlotEmpty { tab_idx, slot_idx } => {
            format!("bank tab {} slot {} is empty", tab_idx, slot_idx)
        }
        BankError::SlotItemMismatch => "item at slot does not match".to_string(),
        BankError::ItemNotInBank { item_id } => format!("{} not found in bank", item_id),
        BankError::InsufficientQuantity { item_id, requested, available } => {
            format!("not enough {}: requested {}, available {}", item_id, requested, available)
        }
        BankError::InventoryFull => "inventory is full".to_string(),
        BankError::TabNotEmpty { tab_idx } => {
            format!("bank tab {} is not empty", tab_idx)
        }
        BankError::Db(e) => {
            warn!("bank db error: {:?}", e);
            "database error".to_string()
        }
    }
}

fn bag_error_message(e: crate::game::sim::inventory::BagError) -> String {
    use crate::game::sim::inventory::BagError;
    match e {
        BagError::InvalidBagSlot(s) => format!("invalid bag slot {}", s),
        BagError::NoBagEquipped { bag_slot } => format!("bag slot {} has no bag equipped", bag_slot),
        BagError::BagAlreadyEquipped { bag_slot } => format!("bag slot {} already has a bag equipped", bag_slot),
        BagError::ItemIsNotABag { item_id } => format!("{} is not a bag", item_id),
        BagError::BagNotEmpty { bag_slot } => format!("bag slot {} is not empty — remove items first", bag_slot),
        BagError::ItemRejectedByFilter { item_id, required_tag } => {
            format!("{} cannot go in that bag (requires {} tag)", item_id, required_tag)
        }
        BagError::BagFull { bag_slot, slots_total } => {
            format!("bag slot {} is full ({} slots)", bag_slot, slots_total)
        }
        BagError::InventoryFull => "inventory is full".to_string(),
        BagError::SlotEmpty { slot_idx } => format!("slot {} is empty", slot_idx),
        BagError::SlotItemMismatch => "item at slot does not match".to_string(),
        BagError::WrongSlotKind { bag_slot } => {
            if bag_slot == 0 {
                "slot 1 only accepts general bags (no item filter)".to_string()
            } else {
                "slot 2 only accepts typed/skill bags".to_string()
            }
        }
        BagError::Db(e) => {
            warn!("bag db error: {:?}", e);
            "database error".to_string()
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
