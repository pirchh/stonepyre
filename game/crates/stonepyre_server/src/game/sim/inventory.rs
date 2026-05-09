use sqlx::PgPool;
use uuid::Uuid;

use crate::game::protocol::{InventoryItemSnapshot, InventorySnapshot};

const BASE_INVENTORY_SLOTS: i64 = 16;

/// Server-owned inventory grant produced by the live game simulation.
///
/// Persistence is handled outside the simulation tick so the game loop can keep
/// world/action state in memory while item ownership is written to Postgres.
#[derive(Clone, Debug)]
pub struct InventoryGrantRequest {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
    pub action: crate::game::protocol::InteractionAction,
    pub target: crate::game::protocol::InteractionTarget,
    pub display_name: String,
    pub node_id: String,
    pub charges_remaining: u32,
}

#[derive(Clone, Debug)]
pub struct InventoryGrantResult {
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
    pub new_quantity: i64,
}

/// Persist an item grant for a character and return the authoritative item count.
///
/// The current database table stores one aggregate quantity per item id. Stack
/// policy is applied when building inventory snapshots so non-stackable content
/// can occupy separate visible inventory slots without requiring the full slot
/// persistence migration yet.
pub async fn grant_character_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryGrantResult, sqlx::Error> {
    let quantity_i64 = i64::from(quantity);

    let new_quantity: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO game.character_inventory (character_id, item_id, quantity)
        VALUES ($1::uuid, $2::text, $3::bigint)
        ON CONFLICT (character_id, item_id)
        DO UPDATE SET
            quantity = game.character_inventory.quantity + EXCLUDED.quantity,
            updated_at = now()
        RETURNING quantity
        "#,
    )
    .bind(character_id)
    .bind(item_id)
    .bind(quantity_i64)
    .fetch_one(pool)
    .await?;

    Ok(InventoryGrantResult {
        character_id,
        item_id: item_id.to_string(),
        quantity,
        new_quantity,
    })
}

/// Load the DB-authoritative inventory snapshot for a character.
///
/// Stackable item rows are exposed as a single stack entry. Non-stackable item
/// rows are expanded into one visible inventory entry per quantity so OSRS-style
/// gathered resources can occupy separate slots while persistence still uses the
/// existing aggregate table.
pub async fn load_character_inventory_snapshot(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<InventorySnapshot, sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, quantity
        FROM game.character_inventory
        WHERE character_id = $1::uuid
          AND quantity > 0
        ORDER BY updated_at ASC, item_id ASC
        "#,
    )
    .bind(character_id)
    .fetch_all(pool)
    .await?;

    Ok(InventorySnapshot {
        character_id,
        items: expand_inventory_rows_for_snapshot(rows),
    })
}

fn expand_inventory_rows_for_snapshot(rows: Vec<(String, i64)>) -> Vec<InventoryItemSnapshot> {
    let content = stonepyre_content::default_content_db();
    let mut items = Vec::new();

    for (item_id, quantity) in rows {
        let quantity = quantity.max(0);
        if quantity == 0 {
            continue;
        }

        let stackable = content
            .items
            .get(&item_id)
            .map(|def| def.stack_policy.can_stack_in(stonepyre_content::items::StorageKind::Inventory))
            .unwrap_or(true);

        if stackable {
            items.push(InventoryItemSnapshot { item_id, quantity });
            continue;
        }

        // The client inventory panel currently owns the visible slot cap. Avoid
        // producing an unbounded snapshot if a legacy aggregate row has grown
        // very large before stack policy was introduced.
        let remaining_visible_slots = BASE_INVENTORY_SLOTS.saturating_sub(items.len() as i64);
        let visible_quantity = quantity.min(remaining_visible_slots);

        for _ in 0..visible_quantity {
            items.push(InventoryItemSnapshot {
                item_id: item_id.clone(),
                quantity: 1,
            });
        }
    }

    items
}
