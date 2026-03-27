use axum::{extract::State, Extension, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    http::middleware::AuthContext,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct TradeRequest {
    pub character_id: Uuid,
    pub portfolio_id: Uuid,
    pub company_id: i64,
    pub side: String, // "BUY" | "SELL"
    pub shares: f64,
    pub fee: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct TradeResponse {
    pub tx_id: Uuid,
    pub executed_price: f64,
}

pub async fn execute_trade(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<TradeRequest>,
) -> Result<Json<TradeResponse>, ApiError> {
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

    // ✅ Server-priced: derive execution price from DB, not from request.
    let price = fetch_authoritative_price(&state, req.company_id).await?;

    let tx_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT portfolio.execute_trade(
            $1::text,
            $2::uuid,
            $3::uuid,
            $4::bigint,
            $5::text,
            $6::numeric,
            $7::numeric,
            $8::numeric
        )
        "#,
    )
    .bind(&auth.token)
    .bind(req.character_id)
    .bind(req.portfolio_id)
    .bind(req.company_id)
    .bind(&req.side)
    .bind(req.shares)
    .bind(price)
    .bind(fee)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("portfolio.execute_trade failed: {:?}", e);
        ApiError::db(e)
    })?;

    Ok(Json(TradeResponse {
        tx_id,
        executed_price: price,
    }))
}

/// Authoritative execution price:
/// 1) market.v_latest_prices (best, if you have it)
/// 2) market.prices latest row (backup)
/// 3) market.candles_1h close (optional)
/// 4) market.candles close (optional)
async fn fetch_authoritative_price(state: &AppState, company_id: i64) -> Result<f64, ApiError> {
    // 1) View: latest price per company
    if let Some(p) = fetch_optional_ignore_missing::<f64>(
        &state.db,
        r#"
        SELECT lp.price::double precision
        FROM market.v_latest_prices lp
        WHERE lp.company_id = $1
        "#,
        company_id,
        "v_latest_prices",
    )
    .await?
    {
        if p > 0.0 {
            return Ok(p);
        }
    }

    // 2) Backup: directly from market.prices
    if let Some(p) = fetch_optional_ignore_missing::<f64>(
        &state.db,
        r#"
        SELECT p.price::double precision
        FROM market.prices p
        WHERE p.company_id = $1
        ORDER BY p.ts DESC
        LIMIT 1
        "#,
        company_id,
        "prices",
    )
    .await?
    {
        if p > 0.0 {
            return Ok(p);
        }
    }

    // 3) Optional: candles_1h close
    if let Some(p) = fetch_optional_ignore_missing::<f64>(
        &state.db,
        r#"
        SELECT c.close::double precision
        FROM market.candles_1h c
        WHERE c.company_id = $1
        ORDER BY c.sim_day DESC
        LIMIT 1
        "#,
        company_id,
        "candles_1h",
    )
    .await?
    {
        if p > 0.0 {
            return Ok(p);
        }
    }

    // 4) Optional: candles close
    if let Some(p) = fetch_optional_ignore_missing::<f64>(
        &state.db,
        r#"
        SELECT c.close::double precision
        FROM market.candles c
        WHERE c.company_id = $1
        ORDER BY c.sim_day DESC
        LIMIT 1
        "#,
        company_id,
        "candles",
    )
    .await?
    {
        if p > 0.0 {
            return Ok(p);
        }
    }

    Err(ApiError::BadRequest(format!(
        "no market price found for company_id={}",
        company_id
    )))
}

/// Query helper that:
/// - returns Ok(None) if the table/view doesn't exist (SQLSTATE 42P01)
/// - otherwise returns Ok(Some(value)) / Ok(None) normally
/// - otherwise returns Err(ApiError::Db)
async fn fetch_optional_ignore_missing<T>(
    db: &sqlx::PgPool,
    sql: &str,
    company_id: i64,
    label: &'static str,
) -> Result<Option<T>, ApiError>
where
    T: Send + Unpin + for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    let res = sqlx::query_scalar::<_, T>(sql)
        .bind(company_id)
        .fetch_optional(db)
        .await;

    match res {
        Ok(v) => Ok(v),
        Err(e) => {
            if is_undefined_table(&e) {
                tracing::warn!("price lookup skipped missing table/view: {}", label);
                Ok(None)
            } else {
                tracing::error!("price lookup failed ({}): {:?}", label, e);
                Err(ApiError::db(e))
            }
        }
    }
}

fn is_undefined_table(e: &sqlx::Error) -> bool {
    // Postgres: SQLSTATE 42P01 = undefined_table
    match e {
        sqlx::Error::Database(db_err) => db_err.code().as_deref() == Some("42P01"),
        _ => false,
    }
}