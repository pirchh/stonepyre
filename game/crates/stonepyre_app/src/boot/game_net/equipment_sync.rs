use bevy::prelude::*;

use stonepyre_content::items::EquipSlot;
use stonepyre_engine::plugins::inventory::Equipment;
use stonepyre_engine::plugins::world::Player;

use super::status::GameNetStatus;

/// Applies the server's equipment snapshot to the player's `Equipment` component
/// whenever it changes. The component is what the character-tab paper-doll reads;
/// harvest gating is enforced server-side from the DB, not from this component.
pub fn sync_equipment_from_server(
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<&mut Equipment, With<Player>>,
) {
    if !status.equipment_dirty {
        return;
    }
    let Ok(mut equipment) = player_q.single_mut() else {
        return;
    };

    // Rebuild from the snapshot: clear every slot, then apply what's equipped.
    *equipment = Equipment::default();
    for entry in &status.equipment {
        if let Some(slot) = equip_slot_from_id(&entry.slot) {
            equipment.set_slot(slot, Some(entry.item_id.clone()));
        }
    }

    status.equipment_dirty = false;
}

/// Inverse of the server's `equip_slot_id`.
fn equip_slot_from_id(id: &str) -> Option<EquipSlot> {
    Some(match id {
        "helm" => EquipSlot::Helm,
        "shoulders" => EquipSlot::Shoulders,
        "neck" => EquipSlot::Neck,
        "chest" => EquipSlot::Chest,
        "wrist" => EquipSlot::Wrist,
        "gloves" => EquipSlot::Gloves,
        "waist" => EquipSlot::Waist,
        "pants" => EquipSlot::Pants,
        "boots" => EquipSlot::Boots,
        "ring1" => EquipSlot::Ring1,
        "ring2" => EquipSlot::Ring2,
        "back" => EquipSlot::Back,
        "main_hand" => EquipSlot::MainHand,
        _ => return None,
    })
}
