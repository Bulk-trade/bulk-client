use serde::Deserialize;


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

