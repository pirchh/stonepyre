use chrono::{Duration, NaiveDateTime, Utc};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone)]
pub struct SimClock {
    pub sim_day: i32,
    pub minute_of_day: i32,
    pub season: String,
    pub is_open: bool,

    // config
    pub day_length_minutes: i32,
    pub market_close_minutes: i32,
    pub days_per_year: i32,
    pub season_length_days: i32,

    // transitions for this advance
    pub day_rolled: bool,
    pub just_opened: bool,
    pub just_closed: bool,

    // helpful for candle finalize
    pub prev_sim_day: i32,
    pub prev_minute_of_day: i32,

    // NEW: how many real minutes elapsed since last advance (0 => no sim progression)
    pub elapsed_minutes: i32,
}

pub async fn advance_clock(pool: &PgPool) -> anyhow::Result<SimClock> {
    let mut tx = pool.begin().await?;

    let r = sqlx::query(
        r#"
        SELECT
            sim_day,
            minute_of_day,
            day_length_minutes,
            market_close_minutes,
            days_per_year,
            season_length_days,
            last_advance_at,
            is_open
        FROM market.clock
        WHERE id = 1
        FOR UPDATE
        "#,
    )
    .fetch_one(&mut *tx)
    .await?;

    let sim_day: i32 = r.try_get("sim_day")?;
    let minute_of_day: i32 = r.try_get("minute_of_day")?;
    let day_length_minutes: i32 = r.try_get("day_length_minutes")?;
    let market_close_minutes: i32 = r.try_get("market_close_minutes")?;
    let days_per_year: i32 = r.try_get("days_per_year")?;
    let season_length_days: i32 = r.try_get("season_length_days")?;
    let last_advance_at: NaiveDateTime = r.try_get("last_advance_at")?;
    let prev_is_open: bool = r.try_get("is_open")?;

    let now = Utc::now().naive_utc();

    // how many real minutes elapsed since last clock update
    let elapsed_minutes = (now - last_advance_at).num_minutes().max(0) as i32;

    // If < 1 minute elapsed, return current state without rewriting row.
    if elapsed_minutes == 0 {
        let season_index = (sim_day.rem_euclid(days_per_year)) / season_length_days;
        let season = match season_index {
            0 => "SPRING",
            1 => "SUMMER",
            2 => "FALL",
            _ => "WINTER",
        }
        .to_string();

        let is_open = minute_of_day < (day_length_minutes - market_close_minutes);

        tx.commit().await?;
        return Ok(SimClock {
            sim_day,
            minute_of_day,
            season,
            is_open,
            day_length_minutes,
            market_close_minutes,
            days_per_year,
            season_length_days,
            day_rolled: false,
            just_opened: false,
            just_closed: false,
            prev_sim_day: sim_day,
            prev_minute_of_day: minute_of_day,
            elapsed_minutes,
        });
    }

    let prev_sim_day = sim_day;
    let prev_minute_of_day = minute_of_day;

    let mut new_sim_day = sim_day;
    let mut new_minute = minute_of_day + elapsed_minutes;

    let mut day_rolled = false;
    while new_minute >= day_length_minutes {
        new_minute -= day_length_minutes;
        new_sim_day += 1;
        day_rolled = true;
    }

    let season_index = (new_sim_day.rem_euclid(days_per_year)) / season_length_days;
    let season = match season_index {
        0 => "SPRING",
        1 => "SUMMER",
        2 => "FALL",
        _ => "WINTER",
    }
    .to_string();

    let is_open = new_minute < (day_length_minutes - market_close_minutes);

    let just_opened = (!prev_is_open) && is_open;
    let just_closed = prev_is_open && (!is_open);

    sqlx::query(
        r#"
        UPDATE market.clock
        SET sim_day = $1,
            minute_of_day = $2,
            season = $3::market.season,
            is_open = $4,
            last_advance_at = $5
        WHERE id = 1
        "#,
    )
    .bind(new_sim_day)
    .bind(new_minute)
    .bind(&season)
    .bind(is_open)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(SimClock {
        sim_day: new_sim_day,
        minute_of_day: new_minute,
        season,
        is_open,
        day_length_minutes,
        market_close_minutes,
        days_per_year,
        season_length_days,
        day_rolled,
        just_opened,
        just_closed,
        prev_sim_day,
        prev_minute_of_day,
        elapsed_minutes,
    })
}

/// Compute candle opened_at / closed_at timestamps for a sim_day close event.
pub fn candle_window_for_close(
    now: NaiveDateTime,
    day_length_minutes: i32,
    market_close_minutes: i32,
) -> (NaiveDateTime, NaiveDateTime) {
    let open_minutes = (day_length_minutes - market_close_minutes).max(1) as i64;
    let opened_at = now - Duration::minutes(open_minutes);
    let closed_at = now;
    (opened_at, closed_at)
}