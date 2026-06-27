use bevy::prelude::*;
use stonepyre_ui::bag::BagItemActionQueue;
use stonepyre_ui::bag::BagItemAction;
use stonepyre_ui::character_tab::CharacterEquipActionQueue;
use stonepyre_ui::inventory::{InventoryItemAction, InventoryItemActionQueue};

use super::runtime::{
    send_bag_move_item_to_server,
    send_bag_put_item_to_server,
    send_bag_put_item_to_slot_to_server,
    send_bag_take_item_to_server,
    send_bag_take_item_to_slot_to_server,
    send_drop_item_to_server,
    send_equip_bag_to_server,
    send_equip_item_to_server,
    send_swap_inv_slots_to_server,
    send_unequip_bag_to_server,
    send_unequip_item_to_server,
};
use super::status::GameNetRuntime;

pub fn send_inventory_item_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut queue: ResMut<InventoryItemActionQueue>,
) {
    let actions: Vec<_> = queue.actions.drain(..).collect();

    for request in actions {
        match request.action {
            InventoryItemAction::Drop => {
                let sent = send_drop_item_to_server(
                    &game_net,
                    request.slot_idx,
                    request.item_id.clone(),
                    request.quantity,
                );

                if !sent {
                    warn!(
                        "inventory drop action dropped; websocket is not ready slot_idx={} item_id={} quantity={}",
                        request.slot_idx, request.item_id, request.quantity
                    );
                }
            }
            InventoryItemAction::EquipBag { bag_slot } => {
                let sent = send_equip_bag_to_server(&game_net, request.slot_idx, bag_slot);
                if !sent {
                    warn!(
                        "equip bag action dropped; websocket is not ready slot_idx={} bag_slot={}",
                        request.slot_idx, bag_slot
                    );
                }
            }
            InventoryItemAction::Equip => {
                let sent = send_equip_item_to_server(&game_net, request.slot_idx, request.item_id.clone());
                if !sent {
                    warn!(
                        "equip item action dropped; websocket is not ready slot_idx={} item_id={}",
                        request.slot_idx, request.item_id
                    );
                }
            }
            InventoryItemAction::MoveToSlot { to_slot } => {
                let sent = send_swap_inv_slots_to_server(&game_net, request.slot_idx, to_slot);
                if !sent {
                    warn!(
                        "swap inv slots action dropped; websocket is not ready from={} to={}",
                        request.slot_idx, to_slot
                    );
                }
            }
        }
    }
}

pub fn send_character_equip_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut queue: ResMut<CharacterEquipActionQueue>,
) {
    let slots: Vec<String> = queue.unequip_slots.drain(..).collect();
    for slot in slots {
        let sent = send_unequip_item_to_server(&game_net, slot.clone());
        if !sent {
            warn!("unequip item action dropped; websocket is not ready slot={}", slot);
        }
    }
}

pub fn send_bag_item_actions_to_server(
    game_net: Res<GameNetRuntime>,
    mut queue: ResMut<BagItemActionQueue>,
) {
    let actions: Vec<_> = queue.actions.drain(..).collect();

    for action in actions {
        match action {
            BagItemAction::Take { bag_slot, bag_item_slot_idx } => {
                let sent = send_bag_take_item_to_server(&game_net, bag_slot, bag_item_slot_idx);
                if !sent {
                    warn!("bag take action dropped; websocket not ready bag_slot={} slot_idx={}", bag_slot, bag_item_slot_idx);
                }
            }
            BagItemAction::UnequipBag { bag_slot } => {
                let sent = send_unequip_bag_to_server(&game_net, bag_slot);
                if !sent {
                    warn!("unequip bag action dropped; websocket not ready bag_slot={}", bag_slot);
                }
            }
            BagItemAction::PutItem { bag_slot, inventory_slot_idx } => {
                let sent = send_bag_put_item_to_server(&game_net, bag_slot, inventory_slot_idx);
                if !sent {
                    warn!(
                        "bag put item action dropped; websocket not ready bag_slot={} inv_slot={}",
                        bag_slot, inventory_slot_idx
                    );
                }
            }
            BagItemAction::MoveItem { from_bag_slot, from_item_slot, to_bag_slot } => {
                let sent = send_bag_move_item_to_server(&game_net, from_bag_slot, from_item_slot, to_bag_slot);
                if !sent {
                    warn!(
                        "bag move item action dropped; websocket not ready from_bag={} from_slot={} to_bag={}",
                        from_bag_slot, from_item_slot, to_bag_slot
                    );
                }
            }
            BagItemAction::PutItemToSlot { bag_slot, inventory_slot_idx, bag_item_slot_idx } => {
                let sent = send_bag_put_item_to_slot_to_server(&game_net, bag_slot, inventory_slot_idx, bag_item_slot_idx);
                if !sent {
                    warn!(
                        "bag put item to slot action dropped; websocket not ready bag_slot={} inv_slot={} bag_item_slot={}",
                        bag_slot, inventory_slot_idx, bag_item_slot_idx
                    );
                }
            }
            BagItemAction::TakeToSlot { bag_slot, bag_item_slot_idx, inv_slot_idx } => {
                let sent = send_bag_take_item_to_slot_to_server(&game_net, bag_slot, bag_item_slot_idx, inv_slot_idx);
                if !sent {
                    warn!(
                        "bag take item to slot action dropped; websocket not ready bag_slot={} bag_slot_idx={} inv_slot={}",
                        bag_slot, bag_item_slot_idx, inv_slot_idx
                    );
                }
            }
        }
    }
}
