use std::collections::HashMap;

use chrono::Utc;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sqlx::Acquire;
use tracing::{error, info};

use crate::db::queries;
use crate::sim::{calendar, lifecycle, pricing};
use crate::state::AppState;

#[derive(Default, Debug)]
pub struct SimCache {
    pub last_price: HashMap<i64, f64>,
}

pub async fn run_loop(state: AppState) {
    let mut rng = StdRng::seed_from_u64(state.cfg.sim_seed);

    if let Err(e) = warm_cache(&state).await {
        error!("warm_cache error: {e:?}");
    }

    let tick_ms = state.cfg.tick_ms;
    info!("sim loop running, tick_ms={tick_ms}");

    loop {
        let tick_start = std::time::Instant::now();

        if let Err(e) = tick_once(&state, &mut rng).await {
            error!("tick error: {e:?}");
        }

        let elapsed = tick_start.elapsed();
        let sleep_ms = tick_ms.saturating_sub(elapsed.as_millis() as u64);
        tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
    }
}

async fn warm_cache(state: &AppState) -> anyhow::Result<()> {
    let companies = queries::fetch_active_companies(&state.pool).await?;
    let mut cache = state.sim_cache.write().await;

    for c in companies {
        if let Ok(Some(p)) = queries::fetch_latest_price(&state.pool, c.company_id).await {
            cache.last_price.insert(c.company_id, p);
        }
    }
    Ok(())
}

async fn tick_once(state: &AppState, rng: &mut StdRng) -> anyhow::Result<()> {
    // 1) Advance sim clock (authoritative)
    let clock = calendar::advance_clock(&state.pool).await?;

    // CRITICAL: if sim minute didn't advance, don't generate a market tick.
    // This prevents multiple ticks per sim-minute when tick_ms < 60s.
    if clock.elapsed_minutes == 0 {
        return Ok(());
    }

    let now = Utc::now();
    let now_naive = now.naive_utc();

    // 2) Start DB tx + load active companies
    let companies = queries::fetch_active_companies(&state.pool).await?;
    let mut conn = state.pool.acquire().await?;
    let mut tx = conn.begin().await?;

    // 3) If we JUST CLOSED, finalize candles for the day we just finished, then wipe ticks.
    if clock.just_closed {
        let (opened_at, closed_at) = calendar::candle_window_for_close(
            now_naive,
            clock.day_length_minutes,
            clock.market_close_minutes,
        );

        queries::finalize_day_candles_tx(&mut tx, clock.sim_day, opened_at, closed_at).await?;
        queries::truncate_ticks_tx(&mut tx).await?;

        info!(
            "day close: sim_day={} season={} minute={} -> candles finalized, ticks truncated",
            clock.sim_day, clock.season, clock.minute_of_day
        );

        // -----------------------------------------------------
        // IPO TOP-UP (CLOSE ONLY) - uses shared factory on state
        // -----------------------------------------------------
        let active_count = queries::count_active_companies_tx(&mut tx).await?;
        let min_active = state.cfg.min_active_companies;
        let max_active = state.cfg.max_active_companies;
        let batch_max = state.cfg.ipo_batch_max;

        if active_count < min_active && batch_max > 0 {
            let want = (min_active - active_count).min(batch_max);
            let room = (max_active - active_count).max(0);
            let to_create = want.min(room);

            if to_create > 0 {
                let mut created = 0i64;

                let mut factory_guard = state.company_factory.write().await;
                if let Some(factory) = factory_guard.as_mut() {
                    for _ in 0..to_create {
                        if let Some((company_id, ipo_price)) = factory
                            .ipo_one_close_tx(
                                &mut tx,
                                rng,
                                clock.sim_day,
                                &clock.season,
                                clock.minute_of_day,
                            )
                            .await?
                        {
                            let mut cache = state.sim_cache.write().await;
                            cache.last_price.insert(company_id, ipo_price);
                            created += 1;
                        }
                    }
                } else {
                    // Factory not loaded => IPO spawning disabled
                    if rng.gen_bool(0.10) {
                        info!("close ipo top-up skipped (CompanyFactory not loaded)");
                    }
                }

                if created > 0 {
                    info!(
                        "close ipo top-up: created={} active_before={} min={} max={}",
                        created, active_count, min_active, max_active
                    );
                }
            }
        }
        // -----------------------------------------------------
    }

    // Safety net: if day rolled and we missed a close, finalize previous day as well.
    if clock.day_rolled {
        let prev_day = clock.prev_sim_day;
        if prev_day != clock.sim_day {
            let (opened_at, closed_at) = calendar::candle_window_for_close(
                now_naive,
                clock.day_length_minutes,
                clock.market_close_minutes,
            );
            queries::finalize_day_candles_tx(&mut tx, prev_day, opened_at, closed_at).await?;
        }
    }

    // 4) CLOSED market logic: no tick inserts, but allow lifecycle (revivals, etc.)
    if !clock.is_open {
        let bankrupt_candidates: Vec<i64> =
            queries::fetch_bankrupt_candidates(&state.pool, 7).await?;

        if let Some(company_id) = lifecycle::pick_revival_candidate(rng, &bankrupt_candidates) {
            if lifecycle::should_revive(rng) {
                queries::revive_company_tx(&mut tx, company_id).await?;
                queries::record_event_tx(
                    &mut tx,
                    company_id,
                    "REVIVE",
                    Some(serde_json::json!({
                        "note": "revived during market closed",
                        "season": clock.season,
                        "sim_day": clock.sim_day,
                        "minute": clock.minute_of_day
                    })),
                )
                .await?;

                let reipo = pricing::seed_reipo_price(rng);
                let mut cache = state.sim_cache.write().await;
                cache.last_price.insert(company_id, reipo);
            }
        }

        if rng.gen_bool(0.02) {
            info!(
                "tick closed: season={} day={} minute={}",
                clock.season, clock.sim_day, clock.minute_of_day
            );
        }

        tx.commit().await?;
        return Ok(());
    }

    // 5) OPEN market: insert ticks + bankruptcies
    let open_minutes = (clock.day_length_minutes - clock.market_close_minutes).max(1) as f64;

    for c in &companies {
        let seasonality = queries::fetch_seasonality_for_industry(
            &state.pool,
            c.industry_id,
            &clock.season,
        )
        .await?;

        let last_price = {
            let cache = state.sim_cache.read().await;
            cache.last_price.get(&c.company_id).copied()
        }
        .unwrap_or_else(|| pricing::seed_initial_price(rng, c.quality_score));

        let eff_vol_day = c.base_volatility * seasonality.vol_mult;

        let next_price = pricing::step_price(
            rng,
            last_price,
            eff_vol_day,
            c.quality_score,
            seasonality.drift_mult,
            open_minutes,
        );

        let mut volume = pricing::step_volume(rng, next_price) as f64;
        volume *= seasonality.volume_mult;
        let volume = volume.max(0.0) as i64;

        queries::insert_tick_tx(&mut tx, c.company_id, clock.sim_day, now, next_price, volume)
            .await?;

        {
            let mut cache = state.sim_cache.write().await;
            cache.last_price.insert(c.company_id, next_price);
        }

        let bankrupt_roll = lifecycle::should_bankrupt(rng, next_price, c.quality_score);
        let season_bankrupt =
            rng.gen_bool(((seasonality.bankrupt_mult - 1.0).max(0.0) * 0.01).min(0.05));

        if bankrupt_roll || season_bankrupt {
            queries::set_company_status_bankrupt_tx(&mut tx, c.company_id).await?;
            queries::record_event_tx(
                &mut tx,
                c.company_id,
                "BANKRUPT",
                Some(serde_json::json!({
                    "price": next_price,
                    "season": clock.season,
                    "sim_day": clock.sim_day,
                    "minute": clock.minute_of_day
                })),
            )
            .await?;

            let mut cache = state.sim_cache.write().await;
            cache.last_price.remove(&c.company_id);
        }
    }

    // 6) Revival logic (open hours too)
    let bankrupt_candidates: Vec<i64> = queries::fetch_bankrupt_candidates(&state.pool, 7).await?;
    if let Some(company_id) = lifecycle::pick_revival_candidate(rng, &bankrupt_candidates) {
        if lifecycle::should_revive(rng) {
            queries::revive_company_tx(&mut tx, company_id).await?;
            queries::record_event_tx(
                &mut tx,
                company_id,
                "REVIVE",
                Some(serde_json::json!({
                    "note": "revived after bankruptcy",
                    "season": clock.season,
                    "sim_day": clock.sim_day,
                    "minute": clock.minute_of_day
                })),
            )
            .await?;

            let reipo = pricing::seed_reipo_price(rng);
            let mut cache = state.sim_cache.write().await;
            cache.last_price.insert(company_id, reipo);
        }
    }

    // 7) Fill queued portfolio orders (OPEN market only).
    let filled = queries::process_open_orders_for_tick_tx(&mut tx, clock.sim_day, clock.minute_of_day)
        .await?;
    if filled > 0 {
        info!(
            "orders filled: count={} sim_day={} minute={}",
            filled, clock.sim_day, clock.minute_of_day
        );
    }

    if rng.gen_bool(0.02) {
        info!(
            "tick open: season={} day={} minute={} active_companies={} bankrupt_candidates={}",
            clock.season,
            clock.sim_day,
            clock.minute_of_day,
            companies.len(),
            bankrupt_candidates.len()
        );
    }

    tx.commit().await?;
    Ok(())
}