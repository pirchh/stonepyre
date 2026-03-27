use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_addr =
            std::env::var("SERVER_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

        // Prefer explicit server var, fallback to market var (common dev setup)
        let database_url = std::env::var("SERVER_DATABASE_URL")
            .or_else(|_| std::env::var("MARKET_DATABASE_URL"))
            .map_err(|_| anyhow!("SERVER_DATABASE_URL (or MARKET_DATABASE_URL fallback) is required"))?;

        Ok(Self {
            bind_addr,
            database_url,
        })
    }
}