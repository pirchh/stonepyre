use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub server_time: DateTime<Utc>,
    pub rows: Vec<MarketRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketRow {
    pub company_id: i64,
    pub name: String,
    pub industry: String,
    pub price: Option<f64>,
    pub ts: Option<DateTime<Utc>>,
}