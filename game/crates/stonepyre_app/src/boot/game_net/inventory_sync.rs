use bevy::prelude::*;

use stonepyre_engine::plugins::inventory::{Inventory, ItemStack};
use stonepyre_engine::plugins::world::Player;

use super::status::GameNetStatus;

/// Mirrors the latest server-owned inventory snapshot into the existing client
/// Inventory component so the current inventory panel can render it.
///
/// Slot positions are server-authoritative. The client should not compact,
/// reorder, or preserve locally-guessed positions here.
pub fn sync_inventory_from_server(
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<&mut Inventory, With<Player>>,
) {
    if !status.inventory_dirty {
        return;
    }

    let Ok(mut inv) = player_q.single_mut() else {
        return;
    };

    let slot_count = inv.container.slots.len().max(status.inventory_slots_total);
    let mut next_slots = vec![None; slot_count];

    for item in &status.inventory_items {
        if item.quantity <= 0 || item.slot_idx >= next_slots.len() {
            continue;
        }

        next_slots[item.slot_idx] = Some(ItemStack {
            id: item.item_id.clone(),
            qty: item.quantity.clamp(1, i64::from(u32::MAX)) as u32,
        });
    }

    inv.container.slots = next_slots;
    status.inventory_dirty = false;
}
