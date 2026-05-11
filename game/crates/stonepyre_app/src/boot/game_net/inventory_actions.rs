use bevy::prelude::*;
use stonepyre_engine::plugins::inventory::Inventory;
use stonepyre_engine::plugins::world::Player;
use stonepyre_ui::inventory::{InventoryItemAction, InventoryItemActionQueue};

use super::runtime::send_drop_item_to_server;
use super::status::GameNetRuntime;

pub fn send_inventory_item_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut queue: ResMut<InventoryItemActionQueue>,
    mut player_q: Query<&mut Inventory, With<Player>>,
) {
    let actions: Vec<_> = queue.actions.drain(..).collect();

    for request in actions {
        match request.action {
            InventoryItemAction::Drop => {
                if send_drop_item_to_server(&game_net, request.item_id.clone(), request.quantity) {
                    clear_local_inventory_slot(&mut player_q, request.slot_idx, &request.item_id);
                } else {
                    warn!(
                        "inventory drop action dropped; websocket is not ready item_id={} quantity={}",
                        request.item_id, request.quantity
                    );
                }
            }
        }
    }
}

fn clear_local_inventory_slot(
    player_q: &mut Query<&mut Inventory, With<Player>>,
    slot_idx: usize,
    item_id: &str,
) {
    let Ok(mut inv) = player_q.single_mut() else {
        return;
    };

    let Some(slot) = inv.container.slots.get_mut(slot_idx) else {
        return;
    };

    if slot
        .as_ref()
        .map(|stack| stack.id.as_str() == item_id)
        .unwrap_or(false)
    {
        *slot = None;
    }
}
