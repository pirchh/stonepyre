use rand::Rng;

pub fn should_bankrupt(rng: &mut impl Rng, price: f64, quality_score: f64) -> bool {
    // v1: if price is very low, small chance to bankrupt
    // quality reduces bankruptcy probability
    if price > 0.75 {
        return false;
    }
    let quality_factor = (2.0 - quality_score.clamp(0.5, 2.0)) * 0.5; // higher quality => lower
    let p = (0.002 + (0.75 - price) * 0.01) * (1.0 + quality_factor); // ~0.2% to maybe ~1%/tick
    rng.gen_bool(p.clamp(0.0, 0.05))
}

pub fn should_revive(rng: &mut impl Rng) -> bool {
    // Very low per tick (tune later)
    rng.gen_bool(0.0005) // 0.05% per tick
}

pub fn pick_revival_candidate(rng: &mut impl Rng, candidates: &[i64]) -> Option<i64> {
    if candidates.is_empty() {
        return None;
    }
    let idx = rng.gen_range(0..candidates.len());
    Some(candidates[idx])
}