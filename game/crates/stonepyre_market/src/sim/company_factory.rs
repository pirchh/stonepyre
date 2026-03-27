use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use rand::{rngs::StdRng, Rng};

use crate::{db::queries, sim::pricing};

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

/// Runtime IPO generator (CLOSE only).
///
/// Does NOT write ticks because ticks are truncated at close.
/// Returns IPO seed price so the sim cache can start it for next open tick.
pub struct CompanyFactory {
    prefixes: Vec<String>,
    suffix_by_code: HashMap<String, Vec<WeightedSuffix>>,
    used_names_session: HashSet<String>,
    industry_map: HashMap<String, i32>,   // code -> industry_id
    industry_weights: Vec<(String, f64)>, // code, weight
}

impl CompanyFactory {
    pub async fn load(pool: &sqlx::PgPool, assets_dir: &Path) -> anyhow::Result<Self> {
        let prefixes_path = assets_dir.join("company_prefixes.json");
        let suffixes_path = assets_dir.join("company_suffixes.json");

        tracing::info!("CompanyFactory loading prefixes: {}", prefixes_path.display());
        let prefixes: Vec<String> = load_json(&prefixes_path)?;

        tracing::info!("CompanyFactory loading suffixes: {}", suffixes_path.display());
        let suffixes: Vec<SuffixJson> = load_json(&suffixes_path)?;

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

        let industry_map = queries::fetch_industry_id_map(pool).await?;
        let industry_weights = queries::fetch_industry_code_weights(pool).await?;

        Ok(Self {
            prefixes,
            suffix_by_code,
            used_names_session: HashSet::new(),
            industry_map,
            industry_weights,
        })
    }

    pub async fn ipo_one_close_tx(
        &mut self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        rng: &mut StdRng,
        sim_day: i32,
        season: &str,
        minute_of_day: i32,
    ) -> anyhow::Result<Option<(i64, f64)>> {
        let ind_code = pick_weighted_industry(rng, &self.industry_weights);
        let Some(&industry_id) = self.industry_map.get(&ind_code) else {
            return Ok(None);
        };

        let suffix_pool = match self.suffix_by_code.get(&ind_code) {
            Some(v) if !v.is_empty() => v,
            _ => return Ok(None),
        };

        let mut name = None;
        for _ in 0..200 {
            let prefix = self.prefixes[rng.gen_range(0..self.prefixes.len())].clone();
            let suffix = pick_weighted_suffix(rng, suffix_pool);
            let candidate = format!("{prefix} {suffix}");
            if self.used_names_session.insert(candidate.clone()) {
                name = Some(candidate);
                break;
            }
        }
        let Some(name) = name else {
            return Ok(None);
        };

        // tightened fundamentals
        let quality = rng.gen_range(0.85..1.20);
        let base_vol = rng.gen_range(0.010..0.060);

        let inserted =
            queries::insert_company_if_new_tx(tx, &name, industry_id, base_vol, quality).await?;
        let Some(company_id) = inserted else {
            return Ok(None);
        };

        let ipo_price = pricing::seed_initial_price(rng, quality);

        queries::record_event_tx(
            tx,
            company_id,
            "IPO",
            Some(serde_json::json!({
                "industry": ind_code,
                "price": ipo_price,
                "sim_day": sim_day,
                "season": season,
                "minute": minute_of_day,
                "note": "runtime ipo created at close"
            })),
        )
        .await?;

        Ok(Some((company_id, ipo_price)))
    }
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

fn pick_weighted_industry(rng: &mut StdRng, pool: &[(String, f64)]) -> String {
    let total: f64 = pool.iter().map(|(_, w)| *w).sum();
    let mut roll = rng.gen_range(0.0..total.max(0.0001));
    for (code, w) in pool {
        roll -= *w;
        if roll <= 0.0 {
            return code.clone();
        }
    }
    pool.last().unwrap().0.clone()
}