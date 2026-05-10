use bevy::prelude::*;
use stonepyre_ui::inventory::{InventoryItemAction, InventoryItemActionQueue};

use super::runtime::send_drop_item_to_server;
use super::status::GameNetRuntime;

pub fn send_inventory_item_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut queue: ResMut<InventoryItemActionQueue>,
) {
    let actions: Vec<_> = queue.actions.drain(..).collect();

    for request in actions {
        match request.action {
            InventoryItemAction::Drop => {
                if !send_drop_item_to_server(&game_net, request.item_id.clone(), request.quantity) {
                    warn!(
                        "inventory drop action dropped; websocket is not ready item_id={} quantity={}",
                        request.item_id, request.quantity
                    );
                }
            }
        }
    }
}
