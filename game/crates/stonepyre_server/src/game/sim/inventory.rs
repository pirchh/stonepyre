use sqlx::PgPool;
use uuid::Uuid;

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

/// Persist an item grant for a character and return the authoritative stack count.
///
/// This uses an atomic upsert so repeated successful harvest rolls cannot lose
/// quantity updates when they land close together.
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
