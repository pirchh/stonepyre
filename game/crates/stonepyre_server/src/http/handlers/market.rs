use axum::{extract::State, Json};
use serde_json::json;
use sqlx::Row;

use crate::{error::ApiError, state::AppState};

pub async fn clock(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query(
        r#"
        SELECT sim_day, minute_of_day, season::text AS season, is_open
        FROM market.clock
        WHERE id = 1
        "#,
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| ApiError::Db)?;

    let sim_day: i32 = row.try_get("sim_day").map_err(|_| ApiError::Db)?;
    let minute_of_day: i32 = row.try_get("minute_of_day").map_err(|_| ApiError::Db)?;
    let season: String = row.try_get("season").map_err(|_| ApiError::Db)?;
    let is_open: bool = row.try_get("is_open").map_err(|_| ApiError::Db)?;

    Ok(Json(json!({
        "sim_day": sim_day,
        "minute_of_day": minute_of_day,
        "season": season,
        "is_open": is_open
    })))
}