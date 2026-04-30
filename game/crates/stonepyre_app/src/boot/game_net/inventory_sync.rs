use bevy::prelude::*;

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

    let slot_count = inv.container.slots.len();
    inv.container.slots.clear();
    inv.container.slots.resize(slot_count, None);

    for (idx, item) in status.inventory_items.iter().take(slot_count).enumerate() {
        let qty = item.quantity.clamp(0, i64::from(u32::MAX)) as u32;
        if qty == 0 {
            continue;
        }

        inv.container.slots[idx] = Some(ItemStack {
            id: item.item_id.clone(),
            qty,
        });
    }

    status.inventory_dirty = false;
}
