use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::game::protocol::{
    BankItemSnapshot, BankSnapshot, BankTabSnapshot, InventoryItemSnapshot, InventorySnapshot,
};

// Maximum number of player-created bank tabs (indices 1–11; tab 0 is "All").
const MAX_BANK_TABS: u8 = 11;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BankError {
    Db(sqlx::Error),
    TabNotFound { tab_idx: u8 },
    TabLimitReached,
    CannotModifyAllTab,
    FilterConflict { item_tag: String, existing_tab: u8 },
    SlotEmpty { tab_idx: u8, slot_idx: usize },
    SlotItemMismatch,
    ItemNotInBank { item_id: String },
    InsufficientQuantity { item_id: String, requested: i64, available: i64 },
    InventoryFull,
    TabNotEmpty { tab_idx: u8 },
}

impl From<sqlx::Error> for BankError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

// ---------------------------------------------------------------------------
// Internal row types
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BankTabRow {
    container_id: Uuid,
    tab_idx: i32,
    display_name: String,
    tag_filters: Vec<String>,
}

#[derive(Debug)]
struct BankSlotRow {
    slot_idx: i32,
    item_id: String,
    quantity: i64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Advisory lock shared with inventory so bank + inventory mutations are serialised.
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

fn item_stacks_in_bank(content: &stonepyre_content::ContentDb, item_id: &str) -> bool {
    content
        .items
        .get(item_id)
        .map(|def| {
            def.stack_policy
                .can_stack_in(stonepyre_content::items::StorageKind::Bank)
        })
        .unwrap_or(true) // unknown items default to stackable in bank
}

fn item_tags<'a>(content: &'a stonepyre_content::ContentDb, item_id: &str) -> &'a [String] {
    content
        .items
        .get(item_id)
        .map(|def| def.tags.as_slice())
        .unwrap_or(&[])
}

/// First unused compact slot index within a tab's slots.
fn next_bank_slot(rows: &[BankSlotRow]) -> i32 {
    let occupied: Vec<i32> = rows.iter().map(|r| r.slot_idx).collect();
    let mut idx = 0i32;
    while occupied.contains(&idx) {
        idx += 1;
    }
    idx
}

// ---------------------------------------------------------------------------
// Ensure default tab
// ---------------------------------------------------------------------------

/// Get-or-create the default "General" tab (tab_idx = 1) for a character.
/// Called on first bank open and before every bank operation.
pub async fn ensure_default_bank_tab(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO game.character_containers (
            character_id, kind, container_def_id, display_name, slot_capacity, tab_idx, tag_filters
        )
        VALUES ($1::uuid, 'bank_tab', 'bank_tab_general', 'General', 0, 1, ARRAY[]::text[])
        ON CONFLICT (character_id, tab_idx) WHERE kind = 'bank_tab'
        DO NOTHING
        "#,
    )
    .bind(character_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Load snapshot
// ---------------------------------------------------------------------------

/// Load the full bank snapshot for the character (all physical tabs + their items).
/// Tab 0 ("All") is not stored — the client builds it from the returned tabs.
pub async fn load_bank_snapshot(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<BankSnapshot, sqlx::Error> {
    ensure_default_bank_tab(pool, character_id).await?;

    let tab_rows: Vec<(Uuid, i32, String, Option<Vec<String>>)> = sqlx::query_as(
        r#"
        SELECT container_id, tab_idx, display_name, tag_filters
        FROM game.character_containers
        WHERE character_id = $1::uuid
          AND kind = 'bank_tab'
        ORDER BY tab_idx ASC
        "#,
    )
    .bind(character_id)
    .fetch_all(pool)
    .await?;

    let mut tabs = Vec::new();
    for (container_id, tab_idx, display_name, tag_filters) in tab_rows {
        let item_rows: Vec<(i32, String, i64)> = sqlx::query_as(
            r#"
            SELECT slot_idx, item_id, quantity
            FROM game.character_container_slots
            WHERE container_id = $1::uuid
              AND quantity > 0
            ORDER BY slot_idx ASC
            "#,
        )
        .bind(container_id)
        .fetch_all(pool)
        .await?;

        tabs.push(BankTabSnapshot {
            character_id,
            tab_idx: tab_idx as u8,
            display_name,
            tag_filters: tag_filters.unwrap_or_default(),
            items: item_rows
                .into_iter()
                .map(|(s, i, q)| BankItemSnapshot {
                    slot_idx: s.max(0) as usize,
                    item_id: i,
                    quantity: q,
                })
                .collect(),
        });
    }

    Ok(BankSnapshot { character_id, tabs })
}

/// Load a single tab snapshot (used after mutations to return just the changed tab).
async fn load_tab_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
    container_id: Uuid,
    tab_idx: u8,
    display_name: &str,
    tag_filters: &[String],
) -> Result<BankTabSnapshot, sqlx::Error> {
    let item_rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"
        SELECT slot_idx, item_id, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid
          AND quantity > 0
        ORDER BY slot_idx ASC
        "#,
    )
    .bind(container_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(BankTabSnapshot {
        character_id,
        tab_idx,
        display_name: display_name.to_string(),
        tag_filters: tag_filters.to_vec(),
        items: item_rows
            .into_iter()
            .map(|(s, i, q)| BankItemSnapshot {
                slot_idx: s.max(0) as usize,
                item_id: i,
                quantity: q,
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Tab CRUD
// ---------------------------------------------------------------------------

/// Create a new bank tab. Fails if MAX_BANK_TABS reached or filters conflict.
pub async fn bank_create_tab(
    pool: &PgPool,
    character_id: Uuid,
    display_name: &str,
    tag_filters: Vec<String>,
) -> Result<BankTabSnapshot, BankError> {
    ensure_default_bank_tab(pool, character_id).await?;

    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    let existing: Vec<(Uuid, i32, String, Option<Vec<String>>)> = sqlx::query_as(
        r#"
        SELECT container_id, tab_idx, display_name, tag_filters
        FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab'
        ORDER BY tab_idx ASC
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .fetch_all(&mut *tx)
    .await?;

    if existing.len() >= MAX_BANK_TABS as usize {
        return Err(BankError::TabLimitReached);
    }

    // Check filter exclusivity.
    for new_tag in &tag_filters {
        for (_, other_idx, _, other_filters) in &existing {
            if let Some(filters) = other_filters {
                if filters.iter().any(|t| t == new_tag) {
                    return Err(BankError::FilterConflict {
                        item_tag: new_tag.clone(),
                        existing_tab: *other_idx as u8,
                    });
                }
            }
        }
    }

    // Pick the next available tab_idx (1–11).
    let used_indices: Vec<i32> = existing.iter().map(|(_, i, _, _)| *i).collect();
    let new_idx = (1..=MAX_BANK_TABS as i32)
        .find(|i| !used_indices.contains(i))
        .ok_or(BankError::TabLimitReached)?;

    let container_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO game.character_containers (
            character_id, kind, container_def_id, display_name, slot_capacity, tab_idx, tag_filters
        )
        VALUES ($1::uuid, 'bank_tab', 'bank_tab_custom', $2::text, 0, $3::int, $4::text[])
        RETURNING container_id
        "#,
    )
    .bind(character_id)
    .bind(display_name)
    .bind(new_idx)
    .bind(&tag_filters)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(BankTabSnapshot {
        character_id,
        tab_idx: new_idx as u8,
        display_name: display_name.to_string(),
        tag_filters,
        items: vec![],
    })
}

/// Rename a tab or change its tag filters. Fails if filters conflict with other tabs.
pub async fn bank_update_tab(
    pool: &PgPool,
    character_id: Uuid,
    tab_idx: u8,
    display_name: &str,
    tag_filters: Vec<String>,
) -> Result<BankTabSnapshot, BankError> {
    if tab_idx == 0 {
        return Err(BankError::CannotModifyAllTab);
    }

    ensure_default_bank_tab(pool, character_id).await?;
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    let all_tabs: Vec<(Uuid, i32, String, Option<Vec<String>>)> = sqlx::query_as(
        r#"
        SELECT container_id, tab_idx, display_name, tag_filters
        FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab'
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .fetch_all(&mut *tx)
    .await?;

    let this_tab = all_tabs
        .iter()
        .find(|(_, idx, _, _)| *idx == tab_idx as i32)
        .ok_or(BankError::TabNotFound { tab_idx })?;
    let container_id = this_tab.0;

    // Check filter exclusivity (excluding this tab itself).
    for new_tag in &tag_filters {
        for (_, other_idx, _, other_filters) in &all_tabs {
            if *other_idx == tab_idx as i32 {
                continue; // skip self
            }
            if let Some(filters) = other_filters {
                if filters.iter().any(|t| t == new_tag) {
                    return Err(BankError::FilterConflict {
                        item_tag: new_tag.clone(),
                        existing_tab: *other_idx as u8,
                    });
                }
            }
        }
    }

    sqlx::query(
        r#"
        UPDATE game.character_containers
        SET display_name = $2::text,
            tag_filters  = $3::text[],
            updated_at   = now()
        WHERE container_id = $1::uuid
        "#,
    )
    .bind(container_id)
    .bind(display_name)
    .bind(&tag_filters)
    .execute(&mut *tx)
    .await?;

    let snapshot = load_tab_snapshot(&mut tx, character_id, container_id, tab_idx, display_name, &tag_filters).await?;
    tx.commit().await?;
    Ok(snapshot)
}

/// Delete a bank tab. All its items are merged into the General tab (tab 1).
/// Fails if trying to delete tab 1 (the default).
pub async fn bank_delete_tab(
    pool: &PgPool,
    character_id: Uuid,
    tab_idx: u8,
) -> Result<BankSnapshot, BankError> {
    if tab_idx == 0 {
        return Err(BankError::CannotModifyAllTab);
    }
    if tab_idx == 1 {
        // Refuse to delete the General tab if it has items.
        // (We could allow it if empty, but keeping General always present is simpler.)
        return Err(BankError::CannotModifyAllTab);
    }

    ensure_default_bank_tab(pool, character_id).await?;
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    // Fetch the tab being deleted.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab' AND tab_idx = $2::int
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .bind(tab_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;
    let (del_container_id,) = row.ok_or(BankError::TabNotFound { tab_idx })?;

    // Fetch the General tab (tab 1) destination.
    let gen_row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab' AND tab_idx = 1
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (gen_container_id,) = gen_row.ok_or(BankError::TabNotFound { tab_idx: 1 })?;

    let content = stonepyre_content::default_content_db();

    // Move all items from the deleted tab into General.
    let del_items: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"
        SELECT slot_idx, item_id, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid AND quantity > 0
        "#,
    )
    .bind(del_container_id)
    .fetch_all(&mut *tx)
    .await?;

    for (_, item_id, qty) in del_items {
        upsert_bank_item(&mut tx, &content, gen_container_id, &item_id, qty).await?;
    }

    // Delete the tab's container (cascades slots via ON DELETE CASCADE).
    sqlx::query(
        r#"DELETE FROM game.character_containers WHERE container_id = $1::uuid"#,
    )
    .bind(del_container_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Return full updated snapshot.
    load_bank_snapshot(pool, character_id).await.map_err(BankError::Db)
}

// ---------------------------------------------------------------------------
// Deposit
// ---------------------------------------------------------------------------

/// Deposit `quantity` of the item in `inv_slot_idx` from inventory into the bank.
/// The server auto-routes to the tab whose tag_filters match the item.
/// If no tab matches, the item goes to the General tab (tab 1).
/// Returns the updated tab snapshot.
pub async fn bank_deposit_item(
    pool: &PgPool,
    character_id: Uuid,
    inv_slot_idx: usize,
    expected_item_id: &str,
    quantity: i64,
) -> Result<BankTabSnapshot, BankError> {
    if quantity <= 0 {
        return Err(BankError::SlotEmpty { tab_idx: 0, slot_idx: inv_slot_idx });
    }

    ensure_default_bank_tab(pool, character_id).await?;
    let content = stonepyre_content::default_content_db();
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    // Get inventory container.
    let inv_container_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid
          AND kind = 'inventory'
          AND parent_container_id IS NULL
        "#,
    )
    .bind(character_id)
    .fetch_one(&mut *tx)
    .await?;

    // Read the inventory slot.
    let inv_row: Option<(String, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid AND slot_idx = $2::int
        FOR UPDATE
        "#,
    )
    .bind(inv_container_id)
    .bind(inv_slot_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((actual_item_id, available)) = inv_row else {
        return Err(BankError::SlotEmpty { tab_idx: 0, slot_idx: inv_slot_idx });
    };

    if actual_item_id != expected_item_id {
        return Err(BankError::SlotItemMismatch);
    }

    let deposit_qty = quantity.min(available);

    // Determine target bank tab.
    let (target_container_id, target_tab) = route_item_to_tab(&mut tx, character_id, &content, &actual_item_id).await?;

    // Deduct from inventory.
    let remaining = available - deposit_qty;
    if remaining == 0 {
        sqlx::query(
            r#"DELETE FROM game.character_container_slots WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
        )
        .bind(inv_container_id)
        .bind(inv_slot_idx as i32)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            r#"UPDATE game.character_container_slots SET quantity = $3::bigint, updated_at = now()
               WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
        )
        .bind(inv_container_id)
        .bind(inv_slot_idx as i32)
        .bind(remaining)
        .execute(&mut *tx)
        .await?;
    }

    // Upsert into bank tab.
    upsert_bank_item(&mut tx, &content, target_container_id, &actual_item_id, deposit_qty).await?;

    let snapshot = load_tab_snapshot(
        &mut tx,
        character_id,
        target_container_id,
        target_tab.tab_idx as u8,
        &target_tab.display_name,
        &target_tab.tag_filters,
    )
    .await?;

    tx.commit().await?;
    Ok(snapshot)
}

/// Deposit every item in the player's inventory into the bank.
/// Returns the full updated bank snapshot.
pub async fn bank_deposit_all(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<BankSnapshot, BankError> {
    ensure_default_bank_tab(pool, character_id).await?;
    let content = stonepyre_content::default_content_db();
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    let inv_container_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid
          AND kind = 'inventory'
          AND parent_container_id IS NULL
        "#,
    )
    .bind(character_id)
    .fetch_one(&mut *tx)
    .await?;

    let inv_items: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT item_id, SUM(quantity)::bigint
        FROM game.character_container_slots
        WHERE container_id = $1::uuid AND quantity > 0
        GROUP BY item_id
        "#,
    )
    .bind(inv_container_id)
    .fetch_all(&mut *tx)
    .await?;

    // Clear inventory.
    sqlx::query(
        r#"DELETE FROM game.character_container_slots WHERE container_id = $1::uuid"#,
    )
    .bind(inv_container_id)
    .execute(&mut *tx)
    .await?;

    // Route each item to its bank tab.
    for (item_id, qty) in inv_items {
        let (target_container_id, _) =
            route_item_to_tab(&mut tx, character_id, &content, &item_id).await?;
        upsert_bank_item(&mut tx, &content, target_container_id, &item_id, qty).await?;
    }

    // Also deposit and clear items from equipped bag slots.
    let bag_container_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid
          AND kind = 'bag_slot'
        "#,
    )
    .bind(character_id)
    .fetch_all(&mut *tx)
    .await?;

    for bag_container_id in bag_container_ids {
        let bag_items: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT item_id, SUM(quantity)::bigint
            FROM game.character_container_slots
            WHERE container_id = $1::uuid AND quantity > 0
            GROUP BY item_id
            "#,
        )
        .bind(bag_container_id)
        .fetch_all(&mut *tx)
        .await?;

        if !bag_items.is_empty() {
            sqlx::query(
                r#"DELETE FROM game.character_container_slots WHERE container_id = $1::uuid"#,
            )
            .bind(bag_container_id)
            .execute(&mut *tx)
            .await?;

            for (item_id, qty) in bag_items {
                let (target_container_id, _) =
                    route_item_to_tab(&mut tx, character_id, &content, &item_id).await?;
                upsert_bank_item(&mut tx, &content, target_container_id, &item_id, qty).await?;
            }
        }
    }

    tx.commit().await?;
    load_bank_snapshot(pool, character_id).await.map_err(BankError::Db)
}

// ---------------------------------------------------------------------------
// Withdraw
// ---------------------------------------------------------------------------

/// Withdraw `quantity` of an item from a bank tab slot into inventory.
/// Quantity is clamped to: min(requested, available_in_bank, free_inv_slots capacity).
/// Returns the updated bank tab snapshot.
pub async fn bank_withdraw_item(
    pool: &PgPool,
    character_id: Uuid,
    tab_idx: u8,
    slot_idx: usize,
    expected_item_id: &str,
    quantity: i64,
) -> Result<(BankTabSnapshot, InventorySnapshot), BankError> {
    if quantity <= 0 {
        return Err(BankError::SlotEmpty { tab_idx, slot_idx });
    }

    ensure_default_bank_tab(pool, character_id).await?;
    let content = stonepyre_content::default_content_db();
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    // Get the bank tab container.
    let tab_row: Option<(Uuid, String, Option<Vec<String>>)> = sqlx::query_as(
        r#"
        SELECT container_id, display_name, tag_filters
        FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab' AND tab_idx = $2::int
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .bind(tab_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;
    let (bank_container_id, display_name, tag_filters) =
        tab_row.ok_or(BankError::TabNotFound { tab_idx })?;
    let tag_filters = tag_filters.unwrap_or_default();

    // Locate the item by id, not by slot: the client's `slot_idx` is unreliable
    // (the "All" view renumbers slots for display), so we identify the item by
    // `expected_item_id` and drain across however many slots hold it.
    let matching: Vec<(i32, i64)> = sqlx::query_as(
        r#"
        SELECT slot_idx, quantity
        FROM game.character_container_slots
        WHERE container_id = $1::uuid AND item_id = $2 AND quantity > 0
        ORDER BY slot_idx ASC
        FOR UPDATE
        "#,
    )
    .bind(bank_container_id)
    .bind(expected_item_id)
    .fetch_all(&mut *tx)
    .await?;

    if matching.is_empty() {
        return Err(BankError::ItemNotInBank { item_id: expected_item_id.to_string() });
    }
    let total_available: i64 = matching.iter().map(|(_, q)| *q).sum();

    // Get inventory container and its current state.
    let inv_container_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT container_id FROM game.character_containers
        WHERE character_id = $1::uuid
          AND kind = 'inventory'
          AND parent_container_id IS NULL
        "#,
    )
    .bind(character_id)
    .fetch_one(&mut *tx)
    .await?;

    let inv_rows = load_inv_slot_rows_for_update(&mut tx, inv_container_id).await?;
    let inv_slots_total = 20i64; // BASE_INVENTORY_SLOTS

    // How many can actually fit into inventory?
    let withdraw_qty = clamp_withdraw_to_inventory(
        &content,
        &inv_rows,
        inv_slots_total,
        expected_item_id,
        quantity.min(total_available),
    );

    if withdraw_qty <= 0 {
        return Err(BankError::InventoryFull);
    }

    // Deduct from the bank, draining matching slots lowest-first.
    let mut remaining = withdraw_qty;
    for (slot, slot_qty) in &matching {
        if remaining <= 0 {
            break;
        }
        let take = remaining.min(*slot_qty);
        let new_qty = *slot_qty - take;
        if new_qty == 0 {
            sqlx::query(
                r#"DELETE FROM game.character_container_slots WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
            )
            .bind(bank_container_id)
            .bind(*slot)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"UPDATE game.character_container_slots SET quantity = $3::bigint, updated_at = now()
                   WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
            )
            .bind(bank_container_id)
            .bind(*slot)
            .bind(new_qty)
            .execute(&mut *tx)
            .await?;
        }
        remaining -= take;
    }

    // Grant to inventory (next free slot; the clamp above guaranteed room).
    grant_to_inventory(&mut tx, &content, inv_container_id, &inv_rows, inv_slots_total, expected_item_id, withdraw_qty).await?;

    let bank_snapshot = load_tab_snapshot(&mut tx, character_id, bank_container_id, tab_idx, &display_name, &tag_filters).await?;
    let inv_snapshot = load_inv_snapshot_in_tx(&mut tx, character_id, inv_container_id, inv_slots_total).await?;

    tx.commit().await?;
    Ok((bank_snapshot, inv_snapshot))
}

// ---------------------------------------------------------------------------
// Move between tabs
// ---------------------------------------------------------------------------

/// Manually move an item from one bank tab to another (player re-organisation).
pub async fn bank_move_item(
    pool: &PgPool,
    character_id: Uuid,
    from_tab_idx: u8,
    slot_idx: usize,
    expected_item_id: &str,
    to_tab_idx: u8,
) -> Result<BankSnapshot, BankError> {
    if from_tab_idx == to_tab_idx {
        return Ok(load_bank_snapshot(pool, character_id).await.map_err(BankError::Db)?);
    }
    if from_tab_idx == 0 || to_tab_idx == 0 {
        return Err(BankError::CannotModifyAllTab);
    }

    ensure_default_bank_tab(pool, character_id).await?;
    let content = stonepyre_content::default_content_db();
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;

    let from_row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT container_id FROM game.character_containers
           WHERE character_id = $1::uuid AND kind = 'bank_tab' AND tab_idx = $2::int FOR UPDATE"#,
    )
    .bind(character_id)
    .bind(from_tab_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;
    let (from_container_id,) = from_row.ok_or(BankError::TabNotFound { tab_idx: from_tab_idx })?;

    let to_row: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT container_id FROM game.character_containers
           WHERE character_id = $1::uuid AND kind = 'bank_tab' AND tab_idx = $2::int FOR UPDATE"#,
    )
    .bind(character_id)
    .bind(to_tab_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;
    let (to_container_id,) = to_row.ok_or(BankError::TabNotFound { tab_idx: to_tab_idx })?;

    let item_row: Option<(String, i64)> = sqlx::query_as(
        r#"SELECT item_id, quantity FROM game.character_container_slots
           WHERE container_id = $1::uuid AND slot_idx = $2::int FOR UPDATE"#,
    )
    .bind(from_container_id)
    .bind(slot_idx as i32)
    .fetch_optional(&mut *tx)
    .await?;
    let (actual_item_id, qty) = item_row.ok_or(BankError::SlotEmpty { tab_idx: from_tab_idx, slot_idx })?;
    if actual_item_id != expected_item_id {
        return Err(BankError::SlotItemMismatch);
    }

    // Remove from source tab.
    sqlx::query(
        r#"DELETE FROM game.character_container_slots WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
    )
    .bind(from_container_id)
    .bind(slot_idx as i32)
    .execute(&mut *tx)
    .await?;

    // Upsert into destination tab.
    upsert_bank_item(&mut tx, &content, to_container_id, &actual_item_id, qty).await?;

    tx.commit().await?;
    load_bank_snapshot(pool, character_id).await.map_err(BankError::Db)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Determine which bank container should receive a given item, based on tag filters.
/// Returns (container_id, tab_row).
async fn route_item_to_tab(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
    content: &stonepyre_content::ContentDb,
    item_id: &str,
) -> Result<(Uuid, TabRouteInfo), BankError> {
    let tabs: Vec<(Uuid, i32, String, Option<Vec<String>>)> = sqlx::query_as(
        r#"
        SELECT container_id, tab_idx, display_name, tag_filters
        FROM game.character_containers
        WHERE character_id = $1::uuid AND kind = 'bank_tab'
        ORDER BY tab_idx ASC
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .fetch_all(&mut **tx)
    .await?;

    let tags = item_tags(content, item_id);

    // Find the first tab whose filter matches any of the item's tags.
    // Tab 1 (General, empty filters) is the fallback.
    let mut general: Option<(Uuid, TabRouteInfo)> = None;

    for (container_id, tab_idx, display_name, tag_filters) in tabs {
        let filters = tag_filters.unwrap_or_default();
        if tab_idx == 1 {
            general = Some((
                container_id,
                TabRouteInfo { tab_idx: tab_idx as u8, display_name, tag_filters: filters },
            ));
            continue;
        }
        if !filters.is_empty() && filters.iter().any(|f| tags.iter().any(|t| t == f)) {
            return Ok((
                container_id,
                TabRouteInfo { tab_idx: tab_idx as u8, display_name, tag_filters: filters },
            ));
        }
    }

    general.ok_or(BankError::TabNotFound { tab_idx: 1 })
}

struct TabRouteInfo {
    tab_idx: u8,
    display_name: String,
    tag_filters: Vec<String>,
}

/// Insert or merge an item into a bank container (respects stack_in_bank policy).
async fn upsert_bank_item(
    tx: &mut Transaction<'_, Postgres>,
    content: &stonepyre_content::ContentDb,
    container_id: Uuid,
    item_id: &str,
    quantity: i64,
) -> Result<(), sqlx::Error> {
    if item_stacks_in_bank(content, item_id) {
        // Try to merge with an existing stack.
        let updated = sqlx::query_scalar::<_, i32>(
            r#"
            UPDATE game.character_container_slots
            SET quantity = quantity + $3::bigint, updated_at = now()
            WHERE container_id = $1::uuid AND item_id = $2::text
            RETURNING slot_idx
            "#,
        )
        .bind(container_id)
        .bind(item_id)
        .bind(quantity)
        .fetch_optional(&mut **tx)
        .await?;

        if updated.is_none() {
            // No existing stack — find next slot.
            let rows = load_bank_slot_rows(tx, container_id).await?;
            let slot = next_bank_slot(&rows);
            sqlx::query(
                r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
                   VALUES ($1::uuid, $2::int, $3::text, $4::bigint)"#,
            )
            .bind(container_id)
            .bind(slot)
            .bind(item_id)
            .bind(quantity)
            .execute(&mut **tx)
            .await?;
        }
    } else {
        // Non-stackable: insert one row per unit.
        let rows = load_bank_slot_rows(tx, container_id).await?;
        let mut occupied: Vec<i32> = rows.iter().map(|r| r.slot_idx).collect();
        for _ in 0..quantity {
            let slot = next_bank_slot_from_occupied(&occupied);
            sqlx::query(
                r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
                   VALUES ($1::uuid, $2::int, $3::text, 1)"#,
            )
            .bind(container_id)
            .bind(slot)
            .bind(item_id)
            .execute(&mut **tx)
            .await?;
            occupied.push(slot);
        }
    }
    Ok(())
}

fn next_bank_slot_from_occupied(occupied: &[i32]) -> i32 {
    let mut idx = 0i32;
    while occupied.contains(&idx) {
        idx += 1;
    }
    idx
}

async fn load_bank_slot_rows(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
) -> Result<Vec<BankSlotRow>, sqlx::Error> {
    let rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"SELECT slot_idx, item_id, quantity FROM game.character_container_slots
           WHERE container_id = $1::uuid AND quantity > 0 ORDER BY slot_idx"#,
    )
    .bind(container_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(slot_idx, item_id, quantity)| BankSlotRow { slot_idx, item_id, quantity })
        .collect())
}

// ---------------------------------------------------------------------------
// Inventory helpers (duplicated minimally to avoid pub-leaking internals)
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct InvSlotRow {
    slot_idx: i32,
    item_id: String,
    quantity: i64,
}

async fn load_inv_slot_rows_for_update(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
) -> Result<Vec<InvSlotRow>, sqlx::Error> {
    let rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"SELECT slot_idx, item_id, quantity FROM game.character_container_slots
           WHERE container_id = $1::uuid AND quantity > 0 ORDER BY slot_idx FOR UPDATE"#,
    )
    .bind(container_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(s, i, q)| InvSlotRow { slot_idx: s, item_id: i, quantity: q })
        .collect())
}

/// How many of `item_id` can we actually withdraw given current inventory state.
fn clamp_withdraw_to_inventory(
    content: &stonepyre_content::ContentDb,
    inv_rows: &[InvSlotRow],
    inv_slots_total: i64,
    item_id: &str,
    requested: i64,
) -> i64 {
    let stacks = item_stacks_in_bank(content, item_id); // bank stacking == inv stacking for this check
    let inv_stacks_it = content
        .items
        .get(item_id)
        .map(|d| d.stack_policy.can_stack_in(stonepyre_content::items::StorageKind::Inventory))
        .unwrap_or(false);

    if inv_stacks_it {
        // If item already has a stack in inventory, we can always add any amount.
        // If not, we need at least one free slot.
        let has_existing_stack = inv_rows.iter().any(|r| r.item_id == item_id);
        if has_existing_stack {
            requested
        } else {
            let slots_used: i64 = inv_rows.len() as i64;
            if slots_used < inv_slots_total { requested } else { 0 }
        }
    } else {
        // Each unit needs its own slot.
        let slots_used: i64 = inv_rows.iter().map(|r| r.quantity).sum::<i64>().max(inv_rows.len() as i64);
        // More precisely: count occupied slots.
        let occupied_slots = inv_rows.len() as i64;
        let free_slots = (inv_slots_total - occupied_slots).max(0);
        requested.min(free_slots)
    }
    .max(0)
    .max(0)
}

/// Grant items to inventory inside an existing transaction.
async fn grant_to_inventory(
    tx: &mut Transaction<'_, Postgres>,
    content: &stonepyre_content::ContentDb,
    inv_container_id: Uuid,
    inv_rows: &[InvSlotRow],
    inv_slots_total: i64,
    item_id: &str,
    quantity: i64,
) -> Result<(), BankError> {
    let stacks_in_inv = content
        .items
        .get(item_id)
        .map(|d| d.stack_policy.can_stack_in(stonepyre_content::items::StorageKind::Inventory))
        .unwrap_or(false);

    if stacks_in_inv {
        if let Some(existing) = inv_rows.iter().find(|r| r.item_id == item_id) {
            sqlx::query(
                r#"UPDATE game.character_container_slots SET quantity = quantity + $3::bigint, updated_at = now()
                   WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
            )
            .bind(inv_container_id)
            .bind(existing.slot_idx)
            .bind(quantity)
            .execute(&mut **tx)
            .await?;
        } else {
            let slot = next_inv_slot(inv_rows, inv_slots_total)?;
            sqlx::query(
                r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
                   VALUES ($1::uuid, $2::int, $3::text, $4::bigint)"#,
            )
            .bind(inv_container_id)
            .bind(slot)
            .bind(item_id)
            .bind(quantity)
            .execute(&mut **tx)
            .await?;
        }
    } else {
        let mut occupied: Vec<i32> = inv_rows.iter().map(|r| r.slot_idx).collect();
        for _ in 0..quantity {
            let slot = next_inv_slot_from_occupied(&occupied, inv_slots_total)?;
            sqlx::query(
                r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
                   VALUES ($1::uuid, $2::int, $3::text, 1)"#,
            )
            .bind(inv_container_id)
            .bind(slot)
            .bind(item_id)
            .execute(&mut **tx)
            .await?;
            occupied.push(slot);
        }
    }
    Ok(())
}

fn next_inv_slot(rows: &[InvSlotRow], total: i64) -> Result<i32, BankError> {
    let occupied: Vec<i32> = rows.iter().map(|r| r.slot_idx).collect();
    next_inv_slot_from_occupied(&occupied, total)
}

fn next_inv_slot_from_occupied(occupied: &[i32], total: i64) -> Result<i32, BankError> {
    (0..total as i32)
        .find(|s| !occupied.contains(s))
        .ok_or(BankError::InventoryFull)
}

async fn load_inv_snapshot_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    character_id: Uuid,
    inv_container_id: Uuid,
    slots_total: i64,
) -> Result<InventorySnapshot, sqlx::Error> {
    let rows: Vec<(i32, String, i64)> = sqlx::query_as(
        r#"SELECT slot_idx, item_id, quantity FROM game.character_container_slots
           WHERE container_id = $1::uuid AND quantity > 0 ORDER BY slot_idx"#,
    )
    .bind(inv_container_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(InventorySnapshot {
        character_id,
        slots_total: slots_total as usize,
        items: rows
            .into_iter()
            .map(|(s, i, q)| InventoryItemSnapshot {
                slot_idx: s.max(0) as usize,
                item_id: i,
                quantity: q,
            })
            .collect(),
    })
}
