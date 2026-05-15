use bevy::prelude::*;

use stonepyre_engine::plugins::inventory::{BagSlotItem, PlayerBagSlotState, PlayerBagSlots};
use stonepyre_ui::bag::BagUiState;

use super::status::GameNetStatus;

pub fn sync_bag_slots_from_server(
    mut status: ResMut<GameNetStatus>,
    mut bag_slots: ResMut<PlayerBagSlots>,
    mut bag_ui_state: ResMut<BagUiState>,
) {
    if !status.bag_slots_dirty {
        return;
    }

    bag_slots.slots = status
        .bag_slots
        .iter()
        .map(|s| PlayerBagSlotState {
            bag_slot: s.bag_slot,
            equipped_item_id: s.equipped_item_id.clone(),
            slots_total: s.slots_total,
            items: s
                .items
                .iter()
                .map(|i| BagSlotItem {
                    slot_idx: i.slot_idx,
                    item_id: i.item_id.clone(),
                    quantity: i.quantity.max(0) as u32,
                })
                .collect(),
            item_type_filter: s.item_type_filter.clone(),
        })
        .collect();

    status.bag_slots_dirty = false;

    // Any bag data change means the open panels are stale — rebuild them.
    bag_ui_state.needs_rebuild = true;
}
