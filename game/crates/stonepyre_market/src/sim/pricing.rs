use rand::Rng;

pub fn seed_initial_price(rng: &mut impl Rng, quality: f64) -> f64 {
    let base = 10.0 + rng.gen_range(0.0..25.0);
    (base * (0.7 + (quality * 0.3))).max(0.5)
}

pub fn seed_reipo_price(rng: &mut impl Rng) -> f64 {
    1.0 + rng.gen_range(0.0..4.0)
}

pub fn step_volume(rng: &mut impl Rng, price: f64) -> u64 {
    let base = (price * 50.0).max(50.0);
    (base * rng.gen_range(0.5..1.5)) as u64
}

/// Sane price stepping:
/// - volatility is treated as "daily-ish sigma" (0.01..0.06 works well)
/// - converted to per-tick sigma using sqrt(ticks_per_day)
/// - log-return model + per-tick circuit breaker clamp
pub fn step_price(
    rng: &mut impl Rng,
    last_price: f64,
    volatility_day: f64,
    quality: f64,
    drift_mult: f64,
    ticks_per_day: f64,
) -> f64 {
    let ticks = ticks_per_day.max(1.0);
    let q = quality.clamp(0.5, 2.0);

    // small drift per day (quality centered at 1.0)
    let drift_day = (q - 1.0) * 0.01 * drift_mult;
    let drift_tick = drift_day / ticks;

    // daily sigma -> per-tick sigma
    let sigma_tick = (volatility_day / ticks.sqrt()).max(0.00001);

    let z = approx_standard_normal(rng);
    let mut r = drift_tick + z * sigma_tick;

    // circuit breaker: cap % move per tick
    // tuned conservative to stop crazy compounding
    let max_move = 0.03;
    r = r.clamp(-max_move, max_move);

    let mut next = last_price * r.exp();

    if !next.is_finite() {
        next = last_price.max(0.5);
    }
    next = next.max(0.05);
    next
}

fn approx_standard_normal(rng: &mut impl Rng) -> f64 {
    // Irwin–Hall: sum 12 uniforms - 6 ~ N(0,1)
    let mut s = 0.0;
    for _ in 0..12 {
        s += rng.gen_range(0.0..1.0);
    }
    s - 6.0
}