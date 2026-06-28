//! Worn equipment (helm, chest, main-hand, …).
//!
//! Distinct from `inventory` (which models inventory/bank/bag containers): each
//! named slot holds at most one item. The main-hand slot is what gates
//! tool-based harvesting — e.g. an equipped axe whose tier covers a tree.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use stonepyre_content::items::EquipSlot;

use super::inventory::{
    ensure_base_inventory_container, lock_character_inventory, BASE_INVENTORY_SLOTS,
};
use crate::game::protocol::{EquipmentSlotSnapshot, EquipmentSnapshot};

#[derive(Debug)]
pub enum EquipError {
    SlotEmpty { slot_idx: usize },
    NotEquippable { item_id: String },
    /// The character's skill level is too low to wield this tool.
    LevelTooLow { required: u32, skill_display: String },
    InventoryFull,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for EquipError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

/// Stable string id for an equipment slot, used in the DB and the protocol.
pub fn equip_slot_id(slot: EquipSlot) -> &'static str {
    match slot {
        EquipSlot::Helm => "helm",
        EquipSlot::Shoulders => "shoulders",
        EquipSlot::Neck => "neck",
        EquipSlot::Chest => "chest",
        EquipSlot::Wrist => "wrist",
        EquipSlot::Gloves => "gloves",
        EquipSlot::Waist => "waist",
        EquipSlot::Pants => "pants",
        EquipSlot::Boots => "boots",
        EquipSlot::Ring1 => "ring1",
        EquipSlot::Ring2 => "ring2",
        EquipSlot::Back => "back",
        EquipSlot::MainHand => "main_hand",
    }
}

/// Skill that gates *wielding* a tool of the given `kind`, as
/// `(skill_id, display_name)`. Inverse of the harvest side's skill->tool map.
fn wield_skill_for_tool(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "axe" => Some((
            crate::game::sim::skills::WOODCUTTING_SKILL_ID,
            crate::game::sim::skills::WOODCUTTING_DISPLAY_NAME,
        )),
        _ => None,
    }
}

/// Equip a worn item from the main inventory. The destination slot is derived
/// from the item's equipment def; any item already in that slot is swapped back
/// into the freed inventory slot. Returns the full equipment snapshot.
pub async fn equip_item(
    pool: &PgPool,
    character_id: Uuid,
    inventory_slot_idx: usize,
    item_id: &str,
) -> Result<EquipmentSnapshot, EquipError> {
    let content = stonepyre_content::default_content_db();

    // Wield-level gate (fail fast, before locking): tools such as axes require a
    // minimum skill level to equip. Keyed off the client-sent item_id; the
    // find-by-id below still confirms the item is actually in the inventory.
    if let Some(tool) = content.items.get(item_id).and_then(|i| i.tool.as_ref()) {
        if tool.wield_level > 0 {
            if let Some((skill_id, skill_display)) = wield_skill_for_tool(&tool.kind) {
                let progress = crate::game::sim::skills::load_character_skill_progress(
                    pool,
                    character_id,
                    skill_id,
                )
                .await?;
                if progress.level < tool.wield_level {
                    return Err(EquipError::LevelTooLow {
                        required: tool.wield_level,
                        skill_display: skill_display.to_string(),
                    });
                }
            }
        }
    }

    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;
    let inv_container_id = ensure_base_inventory_container(&mut tx, character_id).await?;

    // Resolve by item id, not the client's slot: rapid equips/swaps can shift the
    // inventory under a click, so the slot index is only a hint. Use the lowest
    // slot actually holding the item.
    let inv_row: Option<(i32, String)> = sqlx::query_as(
        r#"
        SELECT slot_idx, item_id
        FROM game.character_container_slots
        WHERE container_id = $1::uuid AND item_id = $2 AND quantity > 0
        ORDER BY slot_idx ASC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(inv_container_id)
    .bind(item_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((slot_i32, item_id)) = inv_row else {
        return Err(EquipError::SlotEmpty { slot_idx: inventory_slot_idx });
    };
    let inventory_slot_idx = slot_i32 as usize;

    let equip_def = content
        .items
        .get(&item_id)
        .and_then(|i| i.equipment.as_ref())
        .ok_or_else(|| EquipError::NotEquippable { item_id: item_id.clone() })?;
    let slot_id = equip_slot_id(equip_def.slot);

    // Item currently in that equipment slot, if any — swapped back to inventory.
    let current: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT item_id FROM game.character_equipment
        WHERE character_id = $1::uuid AND slot = $2::text
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .bind(slot_id)
    .fetch_optional(&mut *tx)
    .await?;

    // Remove the new item from its inventory slot.
    sqlx::query(
        r#"DELETE FROM game.character_container_slots
           WHERE container_id = $1::uuid AND slot_idx = $2::int"#,
    )
    .bind(inv_container_id)
    .bind(inventory_slot_idx as i32)
    .execute(&mut *tx)
    .await?;

    // Swap any previously-equipped item back into the now-free inventory slot.
    if let Some((prev_item,)) = current {
        sqlx::query(
            r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
               VALUES ($1::uuid, $2::int, $3::text, 1)"#,
        )
        .bind(inv_container_id)
        .bind(inventory_slot_idx as i32)
        .bind(&prev_item)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        r#"
        INSERT INTO game.character_equipment (character_id, slot, item_id)
        VALUES ($1::uuid, $2::text, $3::text)
        ON CONFLICT (character_id, slot)
        DO UPDATE SET item_id = EXCLUDED.item_id, updated_at = now()
        "#,
    )
    .bind(character_id)
    .bind(slot_id)
    .bind(&item_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    load_character_equipment(pool, character_id).await
}

/// Unequip the item in the given worn slot back into the main inventory.
pub async fn unequip_item(
    pool: &PgPool,
    character_id: Uuid,
    slot_id: &str,
) -> Result<EquipmentSnapshot, EquipError> {
    let mut tx = pool.begin().await?;
    lock_character_inventory(&mut tx, character_id).await?;
    let inv_container_id = ensure_base_inventory_container(&mut tx, character_id).await?;

    let current: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT item_id FROM game.character_equipment
        WHERE character_id = $1::uuid AND slot = $2::text
        FOR UPDATE
        "#,
    )
    .bind(character_id)
    .bind(slot_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((item_id,)) = current else {
        // Nothing equipped in that slot — return the current state unchanged.
        tx.commit().await?;
        return load_character_equipment(pool, character_id).await;
    };

    let Some(free_slot) = first_free_inventory_slot(&mut tx, inv_container_id).await? else {
        return Err(EquipError::InventoryFull);
    };

    sqlx::query(
        r#"INSERT INTO game.character_container_slots (container_id, slot_idx, item_id, quantity)
           VALUES ($1::uuid, $2::int, $3::text, 1)"#,
    )
    .bind(inv_container_id)
    .bind(free_slot as i32)
    .bind(&item_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DELETE FROM game.character_equipment
           WHERE character_id = $1::uuid AND slot = $2::text"#,
    )
    .bind(character_id)
    .bind(slot_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    load_character_equipment(pool, character_id).await
}

/// Full worn-equipment snapshot for a character.
pub async fn load_character_equipment(
    pool: &PgPool,
    character_id: Uuid,
) -> Result<EquipmentSnapshot, EquipError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT slot, item_id FROM game.character_equipment
           WHERE character_id = $1::uuid ORDER BY slot"#,
    )
    .bind(character_id)
    .fetch_all(pool)
    .await?;

    Ok(EquipmentSnapshot {
        character_id,
        slots: rows
            .into_iter()
            .map(|(slot, item_id)| EquipmentSlotSnapshot { slot, item_id })
            .collect(),
    })
}

/// Outcome of checking whether a character may harvest a node.
pub enum HarvestGate {
    /// Allowed. Carries the inputs the per-swing success scaling needs: the
    /// character's level in the node's skill and the equipped tool's tier.
    Ok { skill_level: u32, tool_level: u32 },
    LevelTooLow { required: u32, skill_display: String },
    ToolMissing { required_tool_name: String },
}

/// Server-authoritative harvest gate: the character must meet the node's skill
/// level requirement AND have an equipped main-hand tool of the right kind whose
/// `harvest_level` covers the node's `required_level`.
pub async fn check_harvest_gate(
    pool: &PgPool,
    character_id: Uuid,
    required_level: u32,
    skill_id: &str,
    skill_display: &str,
    required_tool_kind: &str,
) -> Result<HarvestGate, sqlx::Error> {
    // 1) Skill level.
    let progress =
        crate::game::sim::skills::load_character_skill_progress(pool, character_id, skill_id)
            .await?;
    if progress.level < required_level {
        return Ok(HarvestGate::LevelTooLow {
            required: required_level,
            skill_display: skill_display.to_string(),
        });
    }

    // 2) Equipped main-hand tool of the right kind, covering the node level.
    let content = stonepyre_content::default_content_db();
    let equipped = equipped_item_in_slot(pool, character_id, "main_hand").await?;
    let tool_level = equipped
        .as_ref()
        .and_then(|id| content.items.get(id))
        .and_then(|item| item.tool.as_ref())
        .filter(|tool| tool.kind == required_tool_kind && tool.harvest_level >= required_level)
        .map(|tool| tool.harvest_level);

    let Some(tool_level) = tool_level else {
        return Ok(HarvestGate::ToolMissing {
            required_tool_name: min_tool_name_for_level(&content, required_tool_kind, required_level),
        });
    };

    Ok(HarvestGate::Ok {
        skill_level: progress.level,
        tool_level,
    })
}

/// Display name of the lowest-tier tool of `kind` that can harvest `required_level`
/// (e.g. "Copper Axe"). Falls back to a generic phrase if none is defined.
fn min_tool_name_for_level(
    content: &stonepyre_content::ContentDb,
    kind: &str,
    required_level: u32,
) -> String {
    content
        .items
        .items
        .values()
        .filter_map(|item| item.tool.as_ref().map(|tool| (item, tool)))
        .filter(|(_, tool)| tool.kind == kind && tool.harvest_level >= required_level)
        .min_by_key(|(_, tool)| tool.harvest_level)
        .map(|(item, _)| item.name.clone())
        .unwrap_or_else(|| format!("a better {kind}"))
}

/// The item id equipped in a given slot, if any. Used by harvest gating to read
/// the equipped main-hand tool.
pub async fn equipped_item_in_slot(
    pool: &PgPool,
    character_id: Uuid,
    slot_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT item_id FROM game.character_equipment
           WHERE character_id = $1::uuid AND slot = $2::text"#,
    )
    .bind(character_id)
    .bind(slot_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(item_id,)| item_id))
}

/// Lowest unused slot index in the base inventory, or None if full.
async fn first_free_inventory_slot(
    tx: &mut Transaction<'_, Postgres>,
    container_id: Uuid,
) -> Result<Option<usize>, sqlx::Error> {
    let used: Vec<i32> = sqlx::query_scalar(
        r#"SELECT slot_idx FROM game.character_container_slots WHERE container_id = $1::uuid"#,
    )
    .bind(container_id)
    .fetch_all(&mut **tx)
    .await?;

    let used: std::collections::HashSet<i32> = used.into_iter().collect();
    for i in 0..(BASE_INVENTORY_SLOTS as i32) {
        if !used.contains(&i) {
            return Ok(Some(i as usize));
        }
    }
    Ok(None)
}
