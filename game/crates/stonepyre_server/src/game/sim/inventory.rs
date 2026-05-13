use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::game::protocol::{InventoryItemSnapshot, InventorySnapshot};

const BASE_INVENTORY_SLOTS: i64 = 20;

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
    pub slot_idx: usize,
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

#[derive(Clone, Debug)]
struct ContainerSlotRow {
    slot_idx: i32,
    item_id: String,
    quantity: i64,
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
    InvalidSlot {
        slot_idx: usize,
    },
    SlotEmpty {
        slot_idx: usize,
    },
    SlotItemMismatch {
        slot_idx: usize,
        expected_item_id: String,
        actual_item_id: String,
    },
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
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;
    let container_id = ensure_base_inventory_container(&mut tx, character_id).await?;
    let rows = load_container_slot_rows_for_update(&mut tx, container_id).await?;

    let slots_used = slots_used(&content, &rows);
    let additional_slots_required = additional_slots_required_for_grant(&content, &rows, item_id, quantity);

    tx.commit().await?;

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
pub async fn grant_character_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryGrantResult, InventoryGrantError> {
    if quantity == 0 {
        return Ok(InventoryGrantResult {
            character_id,
            item_id: item_id.to_string(),
            quantity,
            new_quantity: total_item_quantity(pool, character_id, item_id).await.unwrap_or(0),
        });
    }

    let content = stonepyre_content::default_content_db();
    let mut tx = pool.begin().await?;

    lock_character_inventory(&mut tx, character_id).await?;
    let container_id = ensure_base_inventory_container(&mut tx, character_id).await?;
    let rows = load_container_slot_rows_for_update(&mut tx, container_id).await?;

    let slots_used = slots_used(&content, &rows);
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

    if item_stacks_in_inventory(&content, item_id) {
        if let Some(existing) = rows.iter().find(|row| row.item_id == item_id) {
            sqlx::query(
                r#"
                UPDATE game.character_container_slots
                SET quantity = quantity + $3::bigint,
                    updated_at = now()
                WHERE container_id = $1::uuid
                  AND slot_idx = $2::int
                "#,
            )
            .bind(container_id)
            .bind(existing.slot_idx)
            .bind(i64::from(quantity))
            .execute(&mut *tx)
            .await?;
        } else {
            let slot_idx = first_empty_slot(&rows).ok_or_else(|| InventoryGrantError::InventoryFull {
                item_id: item_id.to_string(),
                quantity,
                slots_used,
                slots_total: BASE_INVENTORY_SLOTS,
            })?;
            insert_container_slot(&mut tx, container_id, slot_idx, item_id, i64::from(quantity)).await?;
        }
    } else {
        let mut occupied = rows.iter().map(|row| row.slot_idx).collect::<Vec<_>>();
        for _ in 0..quantity {
            let slot_idx = first_empty_slot_from_occupied(&occupied).ok_or_else(|| {
                InventoryGrantError::InventoryFull {
                    item_id: item_id.to_string(),
                    quantity,
                    slots_used,
                    slots_total: BASE_INVENTORY_SLOTS,
                }
            })?;
            insert_container_slot(&mut tx, container_id, slot_idx, item_id, 1).await?;
            occupied.push(slot_idx);
        }
    }

    sync_aggregate_item_quantity(&mut tx, character_id, container_id, item_id).await?;
    let new_quantity = container_total_item_quantity(&mut tx, container_id, item_id).await?;

    tx.commit().await?;

    Ok(InventoryGrantResult {
        character_id,
        item_id: item_id.to_string(),
        quantity,
        new_quantity,
    })
}

/// Remove an item from the exact visible inventory slot selected by the client.
pub async fn remove_character_item_from_slot(
    pool: &PgPool,
    character_id: Uuid,
    slot_idx: usize,
    expected_item_id: &str,
    quantity: u32,
) -> Result<InventoryRemoveResult, InventoryRemoveError> {
    if quantity == 0 {
        return Err(InventoryRemoveError::InvalidQuantity);
    }
    if slot_idx >= BASE_INVENTORY_SLOTS as usize {
        return Err(InventoryRemoveError::InvalidSlot { slot_idx });
    }

    let mut tx = pool.begin().await?;

    lock_character_inventory(&mut tx, character_id).await?;
    let container_id = ensure_base_inventory_container(&mut tx, character_id).await?;

    let row: Option<(String, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid
          AND slot_idx = $2::int
        FOR UPDATE
        "#,
    )
    .bind(container_id)
    .bind(slot_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((actual_item_id, current_quantity)) = row else {
        tx.rollback().await?;
        return Err(InventoryRemoveError::SlotEmpty { slot_idx });
    };

    if actual_item_id != expected_item_id {
        tx.rollback().await?;
        return Err(InventoryRemoveError::SlotItemMismatch {
            slot_idx,
            expected_item_id: expected_item_id.to_string(),
            actual_item_id,
        });
    }

    let quantity_i64 = i64::from(quantity);
    if current_quantity < quantity_i64 {
        tx.rollback().await?;
        return Err(InventoryRemoveError::InsufficientQuantity {
            item_id: expected_item_id.to_string(),
            requested: quantity,
            available: current_quantity,
        });
    }

    let new_slot_quantity = current_quantity - quantity_i64;
    if new_slot_quantity == 0 {
        sqlx::query(
            r#"
            DELETE FROM game.character_container_slots
            WHERE container_id = $1::uuid
              AND slot_idx = $2::int
            "#,
        )
        .bind(container_id)
        .bind(slot_idx as i32)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE game.character_container_slots
            SET quantity = $3::bigint,
                updated_at = now()
            WHERE container_id = $1::uuid
              AND slot_idx = $2::int
            "#,
        )
        .bind(container_id)
        .bind(slot_idx as i32)
        .bind(new_slot_quantity)
        .execute(&mut *tx)
        .await?;
    }

    sync_aggregate_item_quantity(&mut tx, character_id, container_id, expected_item_id).await?;
    let new_quantity = container_total_item_quantity(&mut tx, container_id, expected_item_id).await?;

    tx.commit().await?;

    Ok(InventoryRemoveResult {
        character_id,
        slot_idx,
        item_id: expected_item_id.to_string(),
        quantity_removed: quantity,
        new_quantity,
    })
}

/// Remove an item from the first slot that contains it. Prefer
/// remove_character_item_from_slot for player inventory interactions.
pub async fn remove_character_item(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
    quantity: u32,
) -> Result<InventoryRemoveResult, InventoryRemoveError> {
    if quantity == 0 {
        return Err(InventoryRemoveError::InvalidQuantity);
    }

    let snapshot = load_character_inventory_snapshot(pool, character_id).await?;
    let Some(item) = snapshot.items.iter().find(|item| item.item_id == item_id) else {
        return Err(InventoryRemoveError::InsufficientQuantity {
            item_id: item_id.to_string(),
            requested: quantity,
            available: 0,
        });
    };

    remove_character_item_from_slot(pool, character_id, item.slot_idx, item_id, quantity).await
}

/// Load the DB-authoritative inventory snapshot for a character.
pub async fn load_character_inventory_snapshot(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<InventorySnapshot, sqlx::Error> {
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;
    let container_id = ensure_base_inventory_container(&mut tx, character_id).await?;
    let rows = load_container_slot_rows_for_update(&mut tx, container_id).await?;
    tx.commit().await?;

    Ok(InventorySnapshot {
        character_id,
        slots_total: BASE_INVENTORY_SLOTS as usize,
        items: rows
            .into_iter()
            .filter(|row| row.quantity > 0)
            .map(|row| InventoryItemSnapshot {
                slot_idx: row.slot_idx.max(0) as usize,
                item_id: row.item_id,
                quantity: row.quantity,
            })
            .collect(),
    })
}

/// Get or create the base inventory container instance for a character.
///
/// Returns the container_id uuid. Safe to call repeatedly — uses INSERT ... ON
/// CONFLICT so concurrent callers converge on the same row.
async fn ensure_base_inventory_container(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO game.character_containers (
            character_id,
            kind,
            container_def_id,
            display_name,
            slot_capacity
        )
        VALUES ($1::uuid, 'inventory', 'base_inventory', 'Inventory', $2::int)
        ON CONFLICT (character_id) WHERE kind = 'inventory' AND parent_container_id IS NULL
        DO UPDATE SET updated_at = game.character_containers.updated_at
        RETURNING container_id
        "#,
    )
    .bind(character_id)
    .bind(BASE_INVENTORY_SLOTS as i32)
    .fetch_one(&mut **tx)
    .await
}

async fn lock_character_inventory(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1::text)::bigint)")
        .bind(character_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn load_container_slot_rows_for_update(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
) -> Result<Vec<ContainerSlotRow>, sqlx::Error> {
    let rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"
        SELECT slot_idx, item_id, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid
          AND quantity > 0
        ORDER BY slot_idx ASC
        FOR UPDATE
        "#,
    )
    .bind(container_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(slot_idx, item_id, quantity)| ContainerSlotRow {
            slot_idx,
            item_id,
            quantity,
        })
        .collect())
}

async fn insert_container_slot(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
    slot_idx: i32,
    item_id: &str,
    quantity: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO game.character_container_slots (
            container_id,
            slot_idx,
            item_id,
            quantity
        )
        VALUES ($1::uuid, $2::int, $3::text, $4::bigint)
        "#,
    )
    .bind(container_id)
    .bind(slot_idx)
    .bind(item_id)
    .bind(quantity)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn sync_aggregate_item_quantity(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
    container_id: Uuid,
    item_id: &str,
) -> Result<(), sqlx::Error> {
    let total = container_total_item_quantity(tx, container_id, item_id).await?;

    if total <= 0 {
        sqlx::query(
            r#"
            DELETE FROM game.character_inventory
            WHERE character_id = $1::uuid
              AND item_id = $2::text
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            r#"
            INSERT INTO game.character_inventory (character_id, item_id, quantity)
            VALUES ($1::uuid, $2::text, $3::bigint)
            ON CONFLICT (character_id, item_id)
            DO UPDATE SET
                quantity = EXCLUDED.quantity,
                updated_at = now()
            "#,
        )
        .bind(character_id)
        .bind(item_id)
        .bind(total)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn container_total_item_quantity(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
    item_id: &str,
) -> Result<i64, sqlx::Error> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(quantity), 0)::bigint
        FROM game.character_container_slots
        WHERE container_id = $1::uuid
          AND item_id = $2::text
        "#,
    )
    .bind(container_id)
    .bind(item_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(total)
}

async fn total_item_quantity(
    pool: &PgPool,
    character_id: Uuid,
    item_id: &str,
) -> Result<i64, sqlx::Error> {
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(quantity), 0)::bigint
        FROM game.character_inventory
        WHERE character_id = $1::uuid
          AND item_id = $2::text
        "#,
    )
    .bind(character_id)
    .bind(item_id)
    .fetch_one(pool)
    .await?;

    Ok(total)
}

fn slots_used(content: &stonepyre_content::ContentDb, rows: &[ContainerSlotRow]) -> i64 {
    rows.iter()
        .filter(|row| row.quantity > 0)
        .map(|row| {
            if item_stacks_in_inventory(content, &row.item_id) {
                1
            } else {
                row.quantity
            }
        })
        .sum()
}

fn additional_slots_required_for_grant(
    content: &stonepyre_content::ContentDb,
    rows: &[ContainerSlotRow],
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
        .any(|row| row.item_id == item_id && row.quantity > 0);

    if existing_stack { 0 } else { 1 }
}

fn first_empty_slot(rows: &[ContainerSlotRow]) -> Option<i32> {
    let occupied = rows.iter().map(|row| row.slot_idx).collect::<Vec<_>>();
    first_empty_slot_from_occupied(&occupied)
}

fn first_empty_slot_from_occupied(occupied: &[i32]) -> Option<i32> {
    (0..BASE_INVENTORY_SLOTS as i32).find(|slot_idx| !occupied.contains(slot_idx))
}

fn item_stacks_in_inventory(content: &stonepyre_content::ContentDb, item_id: &str) -> bool {
    content
        .items
        .get(item_id)
        .map(|def| def.stack_policy.can_stack_in(stonepyre_content::items::StorageKind::Inventory))
        .unwrap_or(true)
}
