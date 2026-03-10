use std::sync::Arc;
use serde::{Deserialize, Serialize};


/// Per-market configuration returned by the exchange.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct MarketInfo {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub status: String,
    pub price_precision: u32,
    pub size_precision: u32,
    pub tick_size: f64,
    pub lot_size: f64,
    pub min_notional: f64,
    pub max_leverage: f64,
    pub order_types: Vec<String>,
    pub time_in_forces: Vec<String>,
}

/// Beacon tx
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Beacon {
    pub epoch: u32,
    pub node_id: u16,
    pub wall_clock_ns: u64,
    pub since_commit_us: u64,
}

/// Add new market tx
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMarket {
    #[serde(rename = "c")]
    pub symbol: Arc<str>,
}