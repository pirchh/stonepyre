use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketConfig {
    pub database_url: String,
    pub bind_addr: String,
    pub tick_ms: u64,
    pub sim_seed: u64,

    // NEW: ecosystem bounds
    pub min_active_companies: i64,     // hard floor (never allow below)
    pub target_active_companies: i64,  // soft target (try to drift toward this)
    pub max_active_companies: i64,     // hard cap (never exceed)

    // NEW: IPO controls
    pub ipo_batch_max: i64,            // max new companies per close

    // NEW: runtime assets path (prefix/suffix JSON)
    pub assets_dir: String,
}

impl MarketConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("MARKET_DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("MARKET_DATABASE_URL is required"))?;

        let bind_addr = std::env::var("MARKET_BIND")
            .unwrap_or_else(|_| "127.0.0.1:7777".to_string());

        let tick_ms = std::env::var("MARKET_TICK_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1000);

        let sim_seed = std::env::var("MARKET_SIM_SEED")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(42);

        // bounds
        let min_active_companies = std::env::var("MARKET_MIN_ACTIVE")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(175);

        let target_active_companies = std::env::var("MARKET_TARGET_ACTIVE")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(220);

        let max_active_companies = std::env::var("MARKET_MAX_ACTIVE")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(300);

        // IPO rate limiting
        let ipo_batch_max = std::env::var("MARKET_IPO_BATCH_MAX")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(8);

        // assets
        let assets_dir = std::env::var("MARKET_ASSETS_DIR")
            .unwrap_or_else(|_| "assets/market".to_string());

        // sanity clamps (don’t panic on bad env values)
        let min_active_companies = min_active_companies.clamp(1, 10_000);
        let max_active_companies = max_active_companies.clamp(min_active_companies, 10_000);
        let target_active_companies =
            target_active_companies.clamp(min_active_companies, max_active_companies);
        let ipo_batch_max = ipo_batch_max.clamp(0, 10_000);

        Ok(Self {
            database_url,
            bind_addr,
            tick_ms,
            sim_seed,
            min_active_companies,
            target_active_companies,
            max_active_companies,
            ipo_batch_max,
            assets_dir,
        })
    }
}