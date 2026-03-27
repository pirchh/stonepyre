use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use chrono::Utc;
use rand::{rngs::StdRng, Rng};

use crate::{db::queries, sim::pricing};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct IndustryJson {
    pub code: String,
    pub name: String,
    pub cap: Option<i32>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SuffixJson {
    pub name: String,
    pub industry: String, // industry code
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
struct WeightedSuffix {
    name: String,
    weight: f64,
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let s = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&s)?)
}

pub async fn seed_all(
    pool: &sqlx::PgPool,
    rng: &mut StdRng,
    assets_dir: &Path,
    fill_ratio: f64, // 0..1
) -> anyhow::Result<()> {
    // Paths
    let industries_path = assets_dir.join("industries.json");
    let prefixes_path = assets_dir.join("company_prefixes.json");
    let suffixes_path = assets_dir.join("company_suffixes.json");

    tracing::info!("loading industries: {}", industries_path.display());
    let industries: Vec<IndustryJson> = load_json(&industries_path)?;

    tracing::info!("loading prefixes: {}", prefixes_path.display());
    let prefixes: Vec<String> = load_json(&prefixes_path)?;

    tracing::info!("loading suffixes: {}", suffixes_path.display());
    let suffixes: Vec<SuffixJson> = load_json(&suffixes_path)?;

    // 1) Upsert industries
    for ind in &industries {
        let w = ind.weight.unwrap_or(1.0);
        queries::upsert_industry(pool, &ind.code, &ind.name, ind.cap, w).await?;
    }

    // 2) Load industry_id map
    let industry_map: HashMap<String, i32> = queries::fetch_industry_id_map(pool).await?;

    // 3) Group suffixes by industry code, with weights
    let mut suffix_by_code: HashMap<String, Vec<WeightedSuffix>> = HashMap::new();
    for s in suffixes {
        suffix_by_code
            .entry(s.industry.clone())
            .or_default()
            .push(WeightedSuffix {
                name: s.name,
                weight: s.weight.unwrap_or(1.0).max(0.0001),
            });
    }

    // 4) Determine sim_day for tick seeding (authoritative from market.clock)
    let sim_day: i32 = queries::fetch_sim_day(pool).await?;

    // 5) Seed companies per industry (respect cap * fill_ratio)
    let mut created_total = 0usize;

    for ind in &industries {
        let Some(&industry_id) = industry_map.get(&ind.code) else {
            continue;
        };

        let cap = ind.cap.unwrap_or(12);
        let target = ((cap as f64) * fill_ratio)
            .round()
            .clamp(1.0, cap as f64) as i32;

        // Already existing count
        let existing = queries::count_companies_in_industry(pool, industry_id).await?;
        let remaining = (target - existing).max(0);

        if remaining == 0 {
            continue;
        }

        let suffix_pool = suffix_by_code.get(&ind.code).cloned().unwrap_or_default();
        if suffix_pool.is_empty() {
            continue;
        }

        let mut used_names: HashSet<String> = HashSet::new();

        for _ in 0..remaining {
            // Try to find a unique name
            let mut name = None;
            for _attempt in 0..200 {
                let prefix = prefixes[rng.gen_range(0..prefixes.len())].clone();
                let suffix = pick_weighted_suffix(rng, &suffix_pool);
                let candidate = format!("{prefix} {suffix}");
                if used_names.insert(candidate.clone()) {
                    name = Some(candidate);
                    break;
                }
            }

            let Some(name) = name else { break; };

            // Random-ish fundamentals
            let quality = rng.gen_range(0.70..1.30);
            let base_vol = rng.gen_range(0.020..0.120);

            // Insert company (ignore if name collision in DB)
            let inserted =
                queries::insert_company_if_new(pool, &name, industry_id, base_vol, quality).await?;
            let Some(company_id) = inserted else { continue; };

            // Seed initial tick (ephemeral) so v_latest_price has something immediately
            let price = pricing::seed_initial_price(rng, quality);
            let volume = pricing::step_volume(rng, price) as i64;

            // ✅ sim_day is i32 now (matches prices_ticks.sim_day INT4)
            queries::insert_tick(pool, company_id, sim_day, Utc::now(), price, volume).await?;

            queries::record_event(
                pool,
                company_id,
                "IPO",
                Some(serde_json::json!({
                    "industry": ind.code,
                    "price": price,
                    "volume": volume,
                    "sim_day": sim_day
                })),
            )
            .await?;

            created_total += 1;
        }
    }

    tracing::info!("seed complete: created_companies={created_total} sim_day={sim_day}");
    Ok(())
}

fn pick_weighted_suffix(rng: &mut StdRng, pool: &[WeightedSuffix]) -> String {
    let total: f64 = pool.iter().map(|s| s.weight).sum();
    let mut roll = rng.gen_range(0.0..total.max(0.0001));
    for s in pool {
        roll -= s.weight;
        if roll <= 0.0 {
            return s.name.clone();
        }
    }
    pool.last().unwrap().name.clone()
}