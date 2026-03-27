use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::http::middleware::AuthContext;
use crate::state::AppState;

#[derive(Debug, Serialize, Clone)]
pub struct Character {
    pub character_id: Uuid,
    pub name: String,
    pub cash: f64,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct CharacterSlots {
    pub slots: [Option<Character>; 5],
}

#[derive(Debug, Deserialize)]
pub struct CreateCharacterReq {
    pub name: String,
}

pub async fn list_slots(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, ApiError> {
    let rows = sqlx::query_as!(
        Character,
        r#"
        SELECT
            character_id,
            name,
            COALESCE(cash, 0)::float8 AS "cash!",
            created_at
        FROM game.characters
        WHERE account_id = $1
        ORDER BY created_at ASC
        LIMIT 5
        "#,
        ctx.account_id
    )
    .fetch_all(&state.db)
    .await?;

    let mut slots: [Option<Character>; 5] = [None, None, None, None, None];
    for (i, c) in rows.into_iter().take(5).enumerate() {
        slots[i] = Some(c);
    }

    Ok(Json(CharacterSlots { slots }))
}

pub async fn create(
    State(state): State<AppState>,
    ctx: AuthContext,
    Json(req): Json<CreateCharacterReq>,
) -> Result<impl IntoResponse, ApiError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".to_string()));
    }

    let created = sqlx::query_as!(
        Character,
        r#"
        INSERT INTO game.characters (account_id, name, cash)
        VALUES ($1, $2, 0)
        RETURNING
            character_id,
            name,
            COALESCE(cash, 0)::float8 AS "cash!",
            created_at
        "#,
        ctx.account_id,
        name
    )
    .fetch_one(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

pub async fn delete(
    State(state): State<AppState>,
    ctx: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let owns: bool = sqlx::query_scalar(
        r#"
        SELECT game.account_owns_character($1::uuid, $2::uuid)
        "#,
    )
    .bind(ctx.account_id)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    if !owns {
        return Err(ApiError::Forbidden);
    }

    let res = sqlx::query(
        r#"
        DELETE FROM game.characters
        WHERE character_id = $1::uuid
          AND account_id = $2::uuid
        "#,
    )
    .bind(id)
    .bind(ctx.account_id)
    .execute(&state.db)
    .await?;

    if res.rows_affected() != 1 {
        return Err(ApiError::Internal("character delete failed".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}