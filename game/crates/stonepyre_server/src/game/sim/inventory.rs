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

#[derive(Clone, Debug)]
pub struct InventoryRemoveResult {
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity_removed: u32,
    pub new_quantity: i64,
}

#[derive(Clone, Debug)]
pub struct InventoryCapacityCheck {
    pub can_accept: bool,
    pub item_id: String,
    pub quantity: u32,
    pub slots_used: i64,
    pub slots_total: i64,
    pub additional_slots_required: i64,
}

#[derive(Debug)]
pub enum InventoryGrantError {
    Db(sqlx::Error),
    InventoryFull {
        item_id: String,
        quantity: u32,
        slots_used: i64,
        slots_total: i64,
    },
}

impl From<sqlx::Error> for InventoryGrantError {
    fn from(value: sqlx::Error) -> Self {
        Self::Db(value)
    }
}

#[derive(Debug)]
pub enum InventoryRemoveError {
    Db(sqlx::Error),
    InvalidQuantity,
    InsufficientQuantity {
        item_id: String,
        requested: u32,
        available: i64,
    },
}

impl From<sqlx::Error> for InventoryRemoveError {
    fn from(value: sqlx::Error) -> Self {
        Self::Db(value)
    }
}

/// Check whether a grant can fit before starting an action.
///
/// This is a preflight check for interaction rejection and UX. The persistent
/// grant path still repeats the check inside a transaction so capacity remains
/// authoritative even if another grant lands between click time and roll time.
pub async fn can_character_inventory_accept_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryCapacityCheck, sqlx::Error> {
    let content = stonepyre_content::default_content_db();
    let rows = load_character_inventory_rows(pool, character_id).await?;
    let slots_used = inventory_slots_used(&content, &rows);
    let additional_slots_required = additional_slots_required_for_grant(&content, &rows, item_id, quantity);

    Ok(InventoryCapacityCheck {
        can_accept: slots_used + additional_slots_required <= BASE_INVENTORY_SLOTS,
        item_id: item_id.to_string(),
        quantity,
        slots_used,
        slots_total: BASE_INVENTORY_SLOTS,
        additional_slots_required,
    })
}

/// Persist an item grant for a character and return the authoritative item count.
///
/// The current database table stores one aggregate quantity per item id. Stack
/// policy is applied before the aggregate write so non-stackable content cannot
/// overflow the visible inventory slot cap before the full slot persistence
/// migration exists.
pub async fn grant_character_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryGrantResult, InventoryGrantError> {
    let quantity_i64 = i64::from(quantity);
    let content = stonepyre_content::default_content_db();

    let mut tx = pool.begin().await?;

    // Serialize inventory grants for this character. Row locks alone are not
    // enough for an empty inventory, because there may be no rows to lock yet.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text)::bigint)")
        .bind(character_id.to_string())
        .execute(&mut *tx)
        .await?;

    let rows: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, quantity
        FROM game.character_inventory
        WHERE character_id = $1::uuid
          AND quantity > 0
        ORDER BY updated_at ASC, item_id ASC
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .fetch_all(&mut *tx)
    .await?;

    let slots_used = inventory_slots_used(&content, &rows);
    let additional_slots = additional_slots_required_for_grant(&content, &rows, item_id, quantity);

    if slots_used + additional_slots > BASE_INVENTORY_SLOTS {
        tx.rollback().await?;
        return Err(InventoryGrantError::InventoryFull {
            item_id: item_id.to_string(),
            quantity,
            slots_used,
            slots_total: BASE_INVENTORY_SLOTS,
        });
    }

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
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(InventoryGrantResult {
        character_id,
        item_id: item_id.to_string(),
        quantity,
        new_quantity,
    })
}

/// Remove items from a character inventory, used by server-authoritative drop.
pub async fn remove_character_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryRemoveResult, InventoryRemoveError> {
    if quantity == 0 {
        return Err(InventoryRemoveError::InvalidQuantity);
    }

    let quantity_i64 = i64::from(quantity);
    let mut tx = pool.begin().await?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text)::bigint)")
        .bind(character_id.to_string())
        .execute(&mut *tx)
        .await?;

    let current_quantity: i64 = sqlx::query_scalar(
        r#"
        SELECT quantity
        FROM game.character_inventory
        WHERE character_id = $1::uuid
          AND item_id = $2::text
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);

    if current_quantity < quantity_i64 {
        tx.rollback().await?;
        return Err(InventoryRemoveError::InsufficientQuantity {
            item_id: item_id.to_string(),
            requested: quantity,
            available: current_quantity,
        });
    }

    let new_quantity = current_quantity - quantity_i64;

    if new_quantity == 0 {
        sqlx::query(
            r#"
            DELETE FROM game.character_inventory
            WHERE character_id = $1::uuid
              AND item_id = $2::text
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE game.character_inventory
            SET quantity = $3::bigint,
                updated_at = now()
            WHERE character_id = $1::uuid
              AND item_id = $2::text
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .bind(new_quantity)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(InventoryRemoveResult {
        character_id,
        item_id: item_id.to_string(),
        quantity_removed: quantity,
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
    let rows = load_character_inventory_rows(pool, character_id).await?;

    Ok(InventorySnapshot {
        character_id,
        items: expand_inventory_rows_for_snapshot(rows),
    })
}

async fn load_character_inventory_rows(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<Vec<(String, i64)>, sqlx::Error> {
    sqlx::query_as(
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
    .await
}

fn expand_inventory_rows_for_snapshot(rows: Vec<(String, i64)>) -> Vec<InventoryItemSnapshot> {
    let content = stonepyre_content::default_content_db();
    let mut items = Vec::new();

    for (item_id, quantity) in rows {
        let quantity = quantity.max(0);
        if quantity == 0 {
            continue;
        }

        if item_stacks_in_inventory(&content, &item_id) {
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

fn inventory_slots_used(
    content: &stonepyre_content::ContentDb,
    rows: &[(String, i64)],
) -> i64 {
    rows.iter()
        .map(|(item_id, quantity)| {
            let quantity = (*quantity).max(0);
            if quantity == 0 {
                0
            } else if item_stacks_in_inventory(content, item_id) {
                1
            } else {
                quantity
            }
        })
        .sum()
}

fn additional_slots_required_for_grant(
    content: &stonepyre_content::ContentDb,
    rows: &[(String, i64)],
    item_id: &str,
    quantity: u32,
) -> i64 {
    if quantity == 0 {
        return 0;
    }

    if !item_stacks_in_inventory(content, item_id) {
        return i64::from(quantity);
    }

    let existing_stack = rows
        .iter()
        .any(|(existing_item_id, existing_quantity)| existing_item_id == item_id && *existing_quantity > 0);

    if existing_stack { 0 } else { 1 }
}

fn item_stacks_in_inventory(content: &stonepyre_content::ContentDb, item_id: &str) -> bool {
    content
        .items
        .get(item_id)
        .map(|def| def.stack_policy.can_stack_in(stonepyre_content::items::StorageKind::Inventory))
        .unwrap_or(true)
}
