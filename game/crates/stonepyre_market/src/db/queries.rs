use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;

use crate::types::{MarketRow, MarketSnapshot};

#[derive(Debug, Clone)]
pub struct ActiveCompany {
    pub company_id: i64,
    pub industry_id: i32,
    pub base_volatility: f64,
    pub quality_score: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Seasonality {
    pub drift_mult: f64,
    pub vol_mult: f64,
    pub volume_mult: f64,
    pub bankrupt_mult: f64,
    pub spawn_mult: f64,
}

pub async fn fetch_seasonality_for_industry(
    pool: &PgPool,
    industry_id: i32,
    season: &str, // "SPRING" | "SUMMER" | "FALL" | "WINTER"
) -> anyhow::Result<Seasonality> {
    let row = sqlx::query(
        r#"
        SELECT drift_mult, vol_mult, volume_mult, bankrupt_mult, spawn_mult
        FROM market.industry_seasonality
        WHERE industry_id = $1
          AND season = $2::market.season
        "#,
    )
    .bind(industry_id)
    .bind(season)
    .fetch_optional(pool)
    .await?;

    if let Some(r) = row {
        Ok(Seasonality {
            drift_mult: r.try_get("drift_mult")?,
            vol_mult: r.try_get("vol_mult")?,
            volume_mult: r.try_get("volume_mult")?,
            bankrupt_mult: r.try_get("bankrupt_mult")?,
            spawn_mult: r.try_get("spawn_mult")?,
        })
    } else {
        Ok(Seasonality {
            drift_mult: 1.0,
            vol_mult: 1.0,
            volume_mult: 1.0,
            bankrupt_mult: 1.0,
            spawn_mult: 1.0,
        })
    }
}

pub async fn fetch_market_snapshot(pool: &PgPool, limit: i64) -> anyhow::Result<MarketSnapshot> {
    let rows = sqlx::query(
        r#"
        SELECT
            company_id,
            name,
            industry,
            price,
            ts
        FROM market.v_market_board
        ORDER BY company_id
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out: Vec<MarketRow> = Vec::with_capacity(rows.len());

    for r in rows {
        let company_id: i64 = r.try_get("company_id")?;
        let name: String = r.try_get("name")?;
        let industry: String = r.try_get("industry")?;

        let price: Option<f64> = r.try_get("price")?;
        let ts_opt: Option<NaiveDateTime> = r.try_get("ts")?;
        let ts = ts_opt.map(|t| DateTime::<Utc>::from_naive_utc_and_offset(t, Utc));

        out.push(MarketRow {
            company_id,
            name,
            industry,
            price,
            ts,
        });
    }

    Ok(MarketSnapshot {
        server_time: Utc::now(),
        rows: out,
    })
}

pub async fn fetch_active_companies(pool: &PgPool) -> anyhow::Result<Vec<ActiveCompany>> {
    let rows = sqlx::query(
        r#"
        SELECT company_id, industry_id, base_volatility, quality_score
        FROM market.companies
        WHERE status = 'ACTIVE' AND listed = TRUE
        ORDER BY company_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());

    for r in rows {
        out.push(ActiveCompany {
            company_id: r.try_get("company_id")?,
            industry_id: r.try_get("industry_id")?,
            base_volatility: r.try_get("base_volatility")?,
            quality_score: r.try_get("quality_score")?,
        });
    }

    Ok(out)
}

/// Latest price for a company (prefers live tick when available, else last candle close).
/// This reads from the unified view.
pub async fn fetch_latest_price(pool: &PgPool, company_id: i64) -> anyhow::Result<Option<f64>> {
    let row = sqlx::query(
        r#"
        SELECT price
        FROM market.v_latest_price
        WHERE company_id = $1
        "#,
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else {
        return Ok(None);
    };

    let price: Option<f64> = r.try_get("price")?;
    Ok(price)
}

/// Fetch current sim_day from the authoritative clock row.
pub async fn fetch_sim_day(pool: &PgPool) -> anyhow::Result<i32> {
    let r = sqlx::query(
        r#"
        SELECT sim_day
        FROM market.clock
        WHERE id = 1
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(r.try_get::<i32, _>("sim_day")?)
}

pub async fn insert_tick_tx(
    tx: &mut Transaction<'_, Postgres>,
    company_id: i64,
    sim_day: i32,
    ts: chrono::DateTime<Utc>,
    price: f64,
    volume: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.prices_ticks (company_id, sim_day, ts, price, volume)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(company_id)
    .bind(sim_day)
    .bind(ts.naive_utc())
    .bind(price)
    .bind(volume)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn finalize_day_candles_tx(
    tx: &mut Transaction<'_, Postgres>,
    sim_day: i32,
    opened_at: NaiveDateTime,
    closed_at: NaiveDateTime,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.candles_1h (
            company_id, sim_day, opened_at, closed_at,
            open, high, low, close, volume
        )
        SELECT
            t.company_id,
            t.sim_day,
            $2::timestamp AS opened_at,
            $3::timestamp AS closed_at,
            (ARRAY_AGG(t.price ORDER BY t.ts ASC))[1]  AS open,
            MAX(t.price)                               AS high,
            MIN(t.price)                               AS low,
            (ARRAY_AGG(t.price ORDER BY t.ts DESC))[1] AS close,
            SUM(t.volume)                              AS volume
        FROM market.prices_ticks t
        WHERE t.sim_day = $1
        GROUP BY t.company_id, t.sim_day
        ON CONFLICT (company_id, sim_day) DO UPDATE
        SET
            opened_at = EXCLUDED.opened_at,
            closed_at = EXCLUDED.closed_at,
            open      = EXCLUDED.open,
            high      = EXCLUDED.high,
            low       = EXCLUDED.low,
            close     = EXCLUDED.close,
            volume    = EXCLUDED.volume
        "#,
    )
    .bind(sim_day)
    .bind(opened_at)
    .bind(closed_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn insert_tick(
    pool: &PgPool,
    company_id: i64,
    sim_day: i32,
    ts: chrono::DateTime<Utc>,
    price: f64,
    volume: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.prices_ticks (company_id, sim_day, ts, price, volume)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(company_id)
    .bind(sim_day)
    .bind(ts.naive_utc())
    .bind(price)
    .bind(volume)
    .execute(pool)
    .await?;

    Ok(())
}

/// Wipe ephemeral ticks (we only keep "today" in ticks).
pub async fn truncate_ticks_tx(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    sqlx::query(r#"TRUNCATE TABLE market.prices_ticks"#)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn record_event_tx(
    tx: &mut Transaction<'_, Postgres>,
    company_id: i64,
    event_type: &str,
    payload: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.events (company_id, ts, event_type, payload)
        VALUES ($1, NOW(), $2, $3)
        "#,
    )
    .bind(company_id)
    .bind(event_type)
    .bind(payload)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn set_company_status_bankrupt_tx(
    tx: &mut Transaction<'_, Postgres>,
    company_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE market.companies
        SET status = 'BANKRUPT',
            listed = FALSE,
            bankrupt_at = NOW()
        WHERE company_id = $1
        "#,
    )
    .bind(company_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn revive_company_tx(
    tx: &mut Transaction<'_, Postgres>,
    company_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE market.companies
        SET status = 'ACTIVE',
            listed = TRUE,
            revived_at = NOW(),
            revival_count = revival_count + 1
        WHERE company_id = $1
        "#,
    )
    .bind(company_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn fetch_bankrupt_candidates(
    pool: &PgPool,
    min_days_bankrupt: i64,
) -> anyhow::Result<Vec<i64>> {
    let rows = sqlx::query(
        r#"
        SELECT company_id
        FROM market.companies
        WHERE status = 'BANKRUPT'
          AND bankrupt_at IS NOT NULL
          AND bankrupt_at <= NOW() - make_interval(days => $1)
        ORDER BY company_id
        "#,
    )
    .bind(min_days_bankrupt as i32)
    .fetch_all(pool)
    .await?;

    let mut out: Vec<i64> = Vec::with_capacity(rows.len());
    for r in rows {
        let company_id: i64 = r.try_get("company_id")?;
        out.push(company_id);
    }
    Ok(out)
}

pub async fn upsert_industry(
    pool: &PgPool,
    code: &str,
    name: &str,
    cap: Option<i32>,
    weight: f64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.industries (code, name, cap, weight)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (code) DO UPDATE
        SET name = EXCLUDED.name,
            cap = EXCLUDED.cap,
            weight = EXCLUDED.weight
        "#,
    )
    .bind(code)
    .bind(name)
    .bind(cap)
    .bind(weight)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_industry_id_map(pool: &PgPool) -> anyhow::Result<HashMap<String, i32>> {
    let rows = sqlx::query(
        r#"
        SELECT industry_id, code
        FROM market.industries
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = HashMap::new();
    for r in rows {
        let id: i32 = r.try_get("industry_id")?;
        let code: String = r.try_get("code")?;
        out.insert(code, id);
    }
    Ok(out)
}

pub async fn count_companies_in_industry(pool: &PgPool, industry_id: i32) -> anyhow::Result<i32> {
    let r = sqlx::query(
        r#"
        SELECT COUNT(*)::INT AS n
        FROM market.companies
        WHERE industry_id = $1
        "#,
    )
    .bind(industry_id)
    .fetch_one(pool)
    .await?;

    Ok(r.try_get::<i32, _>("n")?)
}

pub async fn insert_company_if_new(
    pool: &PgPool,
    name: &str,
    industry_id: i32,
    base_volatility: f64,
    quality_score: f64,
) -> anyhow::Result<Option<i64>> {
    let row = sqlx::query(
        r#"
        INSERT INTO market.companies (name, industry_id, base_volatility, quality_score)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (name) DO NOTHING
        RETURNING company_id
        "#,
    )
    .bind(name)
    .bind(industry_id)
    .bind(base_volatility)
    .bind(quality_score)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.try_get::<i64, _>("company_id")).transpose()?)
}

pub async fn record_event(
    pool: &PgPool,
    company_id: i64,
    event_type: &str,
    payload: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO market.events (company_id, ts, event_type, payload)
        VALUES ($1, NOW(), $2, $3)
        "#,
    )
    .bind(company_id)
    .bind(event_type)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}

// ============================================================
// NEW HELPERS (runtime IPO top-up)
// ============================================================

/// Count ACTIVE + listed companies inside the current tick transaction.
pub async fn count_active_companies_tx(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM market.companies
        WHERE status = 'ACTIVE' AND listed = TRUE
        "#,
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(n)
}

/// Load (industry_code, weight) for weighted runtime IPO picks.
pub async fn fetch_industry_code_weights(pool: &PgPool) -> anyhow::Result<Vec<(String, f64)>> {
    let rows = sqlx::query(
        r#"
        SELECT code, weight::float8 AS weight
        FROM market.industries
        ORDER BY industry_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let code: String = r.try_get("code")?;
        let weight: f64 = r.try_get("weight")?;
        out.push((code, weight.max(0.0001)));
    }
    Ok(out)
}

/// Tx version of insert_company_if_new (for CLOSE-only IPO inserts).
pub async fn insert_company_if_new_tx(
    tx: &mut Transaction<'_, Postgres>,
    name: &str,
    industry_id: i32,
    base_volatility: f64,
    quality_score: f64,
) -> anyhow::Result<Option<i64>> {
    let row = sqlx::query(
        r#"
        INSERT INTO market.companies (name, industry_id, base_volatility, quality_score)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (name) DO NOTHING
        RETURNING company_id
        "#,
    )
    .bind(name)
    .bind(industry_id)
    .bind(base_volatility)
    .bind(quality_score)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.map(|r| r.try_get::<i64, _>("company_id")).transpose()?)
}

// ============================================================
// Portfolio order fills (run inside market tick transaction)
// ============================================================

/// Fill any OPEN portfolio orders using this tick's prices.
/// Should be called ONCE per tick, server-authoritatively, inside the same tx that writes prices_ticks.
pub async fn process_open_orders_for_tick_tx(
    tx: &mut Transaction<'_, Postgres>,
    sim_day: i32,
    minute_of_day: i32,
) -> anyhow::Result<i64> {
    let filled_i32: i32 = sqlx::query_scalar(
        r#"
        SELECT portfolio.process_open_orders_for_tick($1::int, $2::int)
        "#,
    )
    .bind(sim_day)
    .bind(minute_of_day)
    .fetch_one(&mut **tx)
    .await?;

    Ok(filled_i32 as i64)
}

