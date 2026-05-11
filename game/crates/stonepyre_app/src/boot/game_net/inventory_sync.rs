use bevy::prelude::*;
use std::collections::HashMap;

use stonepyre_engine::plugins::inventory::{Inventory, ItemStack};
use stonepyre_engine::plugins::world::Player;

use super::status::GameNetStatus;

/// Mirrors the latest server-owned inventory snapshot/deltas into the existing
/// client Inventory component so the current inventory panel can render it.
///
/// This is presentation state only. The server/DB remain authoritative.
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

    let desired_total_slots: usize = status
        .inventory_items
        .iter()
        .map(|item| item.quantity.max(0) as usize)
        .sum();
    let slot_count = inv.container.slots.len().max(desired_total_slots);

    let mut remaining: HashMap<String, u32> = HashMap::new();
    let mut desired_order: Vec<String> = Vec::new();
    for item in &status.inventory_items {
        let qty = item.quantity.clamp(0, i64::from(u32::MAX)) as u32;
        if qty == 0 {
            continue;
        }
        if !remaining.contains_key(&item.item_id) {
            desired_order.push(item.item_id.clone());
        }
        *remaining.entry(item.item_id.clone()).or_insert(0) += qty;
    }

    let mut next_slots = vec![None; slot_count];

    // Preserve already-rendered slot positions whenever the authoritative item
    // still exists. This keeps OSRS-style inventories from visually compacting
    // after a drop just because the server sent aggregate item counts.
    for (idx, slot) in inv.container.slots.iter().take(slot_count).enumerate() {
        let Some(stack) = slot else {
            continue;
        };
        let Some(left) = remaining.get_mut(&stack.id) else {
            continue;
        };
        if *left == 0 {
            continue;
        }

        next_slots[idx] = Some(ItemStack {
            id: stack.id.clone(),
            qty: 1,
        });
        *left = left.saturating_sub(1);
    }

    // Fill empty slots with any remaining authoritative items. New inventory
    // entries still land in the first available empty slot, but existing items
    // do not jump around.
    for item_id in desired_order {
        let mut left = remaining.remove(&item_id).unwrap_or(0);
        while left > 0 {
            let Some(empty_idx) = next_slots.iter().position(|slot| slot.is_none()) else {
                break;
            };
            next_slots[empty_idx] = Some(ItemStack {
                id: item_id.clone(),
                qty: 1,
            });
            left -= 1;
        }
    }

    inv.container.slots = next_slots;
    status.inventory_dirty = false;
}
