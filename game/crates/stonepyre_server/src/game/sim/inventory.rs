use std::collections::HashMap;

use uuid::Uuid;

/// Server-owned inventory state for the live runtime.
///
/// v0 is intentionally in-memory and keyed by character_id so the shape can move
/// to database persistence later without changing the gameplay authority model.
#[derive(Default)]
pub struct InventoryStore {
    by_character: HashMap<Uuid, Inventory>,
}

impl InventoryStore {
    pub fn ensure_character(&mut self, character_id: Uuid) {
        self.by_character.entry(character_id).or_default();
    }

    pub fn add_item(
        &mut self,
        character_id: Uuid,
        item_id: impl Into<String>,
        quantity: u32,
    ) -> InventoryGrant {
        let item_id = item_id.into();
        let inventory = self.by_character.entry(character_id).or_default();
        let new_quantity = inventory.add_item(item_id.clone(), quantity);

        InventoryGrant {
            character_id,
            item_id,
            quantity,
            new_quantity,
        }
    }
}

#[derive(Default)]
pub struct Inventory {
    stacks: HashMap<String, u32>,
}

impl Inventory {
    fn add_item(&mut self, item_id: String, quantity: u32) -> u32 {
        let entry = self.stacks.entry(item_id).or_insert(0);
        *entry = entry.saturating_add(quantity);
        *entry
    }
}

#[derive(Clone, Debug)]
pub struct InventoryGrant {
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
    pub new_quantity: u32,
}
