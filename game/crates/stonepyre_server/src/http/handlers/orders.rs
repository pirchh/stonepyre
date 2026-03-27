use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    http::middleware::AuthContext,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub character_id: Uuid,
    pub portfolio_id: Uuid,
    pub company_id: i64,
    pub side: String, // BUY / SELL
    pub shares: f64,
    pub fee: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: Uuid,
}

pub async fn place_order(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<PlaceOrderRequest>,
) -> Result<Json<PlaceOrderResponse>, ApiError> {
    if req.shares <= 0.0 {
        return Err(ApiError::BadRequest("shares must be > 0".into()));
    }
    if req.side != "BUY" && req.side != "SELL" {
        return Err(ApiError::BadRequest("side must be BUY or SELL".into()));
    }

    let fee = req.fee.unwrap_or(0.0);
    if fee < 0.0 {
        return Err(ApiError::BadRequest("fee must be >= 0".into()));
    }

    let order_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT portfolio.place_order(
            $1::text,
            $2::uuid,
            $3::uuid,
            $4::bigint,
            $5::text,
            $6::numeric,
            $7::numeric
        )
        "#,
    )
    .bind(&auth.token)
    .bind(req.character_id)
    .bind(req.portfolio_id)
    .bind(req.company_id)
    .bind(&req.side)
    .bind(req.shares)
    .bind(fee)
    .fetch_one(&state.db)
    .await
    .map_err(ApiError::db)?;

    Ok(Json(PlaceOrderResponse { order_id }))
}