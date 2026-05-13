use axum::{extract::State, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, http::middleware::AuthContext, state::AppState};
use crate::game::sim::inventory::{grant_character_item, InventoryGrantError};

#[derive(Debug, Deserialize)]
pub struct GrantItemReq {
    pub character_id: Uuid,
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Debug, Serialize)]
pub struct GrantItemResp {
    pub item_id: String,
    pub quantity: u32,
    pub new_quantity: i64,
}

pub async fn grant_item(
    State(state): State<AppState>,
    _ctx: AuthContext,
    Json(req): Json<GrantItemReq>,
) -> Result<impl IntoResponse, ApiError> {
    if req.item_id.trim().is_empty() {
        return Err(ApiError::BadRequest("item_id is required".to_string()));
    }
    if req.quantity == 0 {
        return Err(ApiError::BadRequest("quantity must be > 0".to_string()));
    }

    let result = grant_character_item(&state.db, req.character_id, &req.item_id, req.quantity)
        .await
        .map_err(|e| match e {
            InventoryGrantError::InventoryFull { .. } => {
                ApiError::BadRequest("inventory is full".to_string())
            }
            InventoryGrantError::Db(db_err) => ApiError::from(db_err),
        })?;

    Ok(Json(GrantItemResp {
        item_id: result.item_id,
        quantity: result.quantity,
        new_quantity: result.new_quantity,
    }))
}
