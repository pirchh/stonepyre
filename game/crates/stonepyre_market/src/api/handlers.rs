use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;

use crate::{db::queries, state::AppState, types::MarketSnapshot};

pub async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
pub struct SnapshotQuery {
    pub limit: Option<i64>,
}

pub async fn snapshot(
    State(state): State<AppState>,
    Query(q): Query<SnapshotQuery>,
) -> Result<Json<MarketSnapshot>, (axum::http::StatusCode, String)> {
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);

    let snap = queries::fetch_market_snapshot(&state.pool, limit)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;

    Ok(Json(snap))
}