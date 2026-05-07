//! Incoming market-data message types.
//!
//! These structs are deserialized from the WebSocket JSON feed and correspond
//! to the Python definitions in `md.py`.

use serde::{Deserialize, Deserializer, Serialize};
use crate::common::side::Side;
use crate::transaction::ActionMeta;

// ============================================================================
// Matrix MD
// ============================================================================


/// Matrix with named columns and rows
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Matrix {
    /// named labels for the matrix
    pub index: Vec<String>,
    pub matrix: Vec<Vec<f64>>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

// ============================================================================
// Summary-level Data
// ============================================================================

/// Market ticker data.
///
/// Received on the `"ticker"` channel.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct Ticker {
    pub symbol: String,
    #[serde(deserialize_with = "f64_or_nan")]
    pub last_price: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub mark_price: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub oracle_price: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub price_change: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub price_change_percent: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub high_price: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub low_price: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub volume: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub quote_volume: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub open_interest: f64,
    #[serde(deserialize_with = "f64_or_nan")]
    pub funding_rate: f64,
}

// ============================================================================
// Candles
// ============================================================================


/// OHLCV candlestick data.
///
/// Received on the `"candle"` channel.
/// The `symbol` and `interval` are populated from the subscription topic,
/// not from the candle payload itself.
#[derive(Debug, Clone,Deserialize)]
#[allow(unused)]
pub struct Candle {
    #[serde(rename="t")]
    pub open_time: u64,
    #[serde(rename = "T")]
    pub close_time: u64,
    #[serde(skip)]
    pub symbol: String,
    #[serde(skip)]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: f64,
    #[serde(rename = "h")]
    pub high: f64,
    #[serde(rename = "l")]
    pub low: f64,
    #[serde(rename = "c")]
    pub close: f64,
    #[serde(rename = "v")]
    pub volume: f64,
    #[serde(rename = "n")]
    pub num_trades: u64,
}


// ============================================================================
// Trades
// ============================================================================

/// A single public trade.
///
/// Received on the `"trades"` channel (as a list).
#[derive(Debug, Clone,Deserialize)]
#[allow(unused)]
pub struct Trade {
    #[serde(rename="time")]
    pub timestamp: u64,
    #[serde(rename="s")]
    pub symbol: String,
    #[serde(rename="b")]
    pub side: Side,
    #[serde(rename="sz")]
    pub size: f64,
    #[serde(rename="px")]
    pub price: f64,
    pub maker: String,
    pub taker: String,
}

// ============================================================================
// Order Book
// ============================================================================

/// Single price level in the order book.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[allow(unused)]
pub struct OrderBookLevel {
    /// Price
    #[serde(rename="px")]
    pub price: f64,
    /// Size (aggregate quantity at this price)
    #[serde(rename="sz")]
    pub size: f64,
    /// Number of orders at this level
    #[serde(rename="n")]
    pub num_orders: u32,
}

impl std::fmt::Display for OrderBookLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.size, self.price)
    }
}


/// Full L2 order-book snapshot.
///
/// Received on the `"l2Snapshot"` channel.
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct L2Snapshot {
    pub timestamp: u64,
    pub symbol: String,
    /// `[[bid_levels], [ask_levels]]`
    pub levels: (Vec<OrderBookLevel>, Vec<OrderBookLevel>),
}


// ============================================================================
// Helpers
// ============================================================================

/// Deserialize an `f64` that may be `null` in JSON, mapping `null` → `NaN`.
pub fn f64_or_nan<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(f64::NAN))
}


// ============================================================================
// Optional SDK integration
// ============================================================================
//
//
// #[cfg(feature = "with-sdk")]
// use bulk_sdk_core::{
//     markets::MktId,
//     data::L2Snapshot as SDKL2Snapshot,
//     data::PriceLevel as SDKPriceLevel,
//     data::L2Delta as SDKL2Delta,
// };
//
// #[cfg(feature = "with-sdk")]
// impl From<&L2Snapshot> for SDKL2Snapshot {
//     fn from(snap: &L2Snapshot) -> Self {
//         let (bids, asks) = &snap.levels;
//
//         let instrument = MktId::new(snap.symbol.as_str()).unwrap();
//         let newbids = bids.iter().map(|x| {
//             SDKPriceLevel {
//                 amount: x.size,
//                 price: x.price,
//                 num_orders: x.num_orders,
//                 cum_vwap: 0.0,
//                 cum_amount: 0.0,
//             }
//         }).collect();
//         let newasks = asks.iter().map(|x| {
//             SDKPriceLevel {
//                 amount: x.size,
//                 price: x.price,
//                 num_orders: x.num_orders,
//                 cum_vwap: 0.0,
//                 cum_amount: 0.0,
//             }
//         }).collect();
//
//         SDKL2Snapshot {
//             stamp: snap.timestamp,
//             instrument,
//             bids: newbids,
//             asks: newasks,
//             trackable_id: Default::default(),
//         }
//     }
// }
//
// #[cfg(feature = "with-sdk")]
// impl From<&OrderBookLevel> for SDKL2Delta {
//     fn from(level: &OrderBookLevel) -> Self {
//         SDKL2Delta {
//             stamp: 0,
//             instrument: Default::default(),
//             side: Default::default(),
//             amount: level.size,
//             price: level.price,
//         }
//     }
// }

//
// Unit Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticker_deserialize_with_nulls() {
        let json = r#"{
            "symbol": "BTC-USD",
            "priceChange": 0.0,
            "priceChangePercent": 0.0,
            "lastPrice": 100000.1824189499,
            "highPrice": 100000.52095557356,
            "lowPrice": 99999.62809703613,
            "volume": 0.0,
            "quoteVolume": 0.0,
            "markPrice": null,
            "oraclePrice": null,
            "openInterest": 0.0,
            "fundingRate": 0.0
        }"#;

        let ticker: Ticker = serde_json::from_str(json).unwrap();

        assert_eq!(ticker.symbol, "BTC-USD");
        assert!((ticker.last_price - 100000.1824189499).abs() < 1e-6);
        assert!((ticker.high_price - 100000.52095557356).abs() < 1e-6);
        assert!((ticker.low_price - 99999.62809703613).abs() < 1e-6);
        assert_eq!(ticker.price_change, 0.0);
        assert_eq!(ticker.price_change_percent, 0.0);
        assert_eq!(ticker.volume, 0.0);
        assert_eq!(ticker.quote_volume, 0.0);
        assert_eq!(ticker.open_interest, 0.0);
        assert_eq!(ticker.funding_rate, 0.0);

        // null → NaN
        assert!(ticker.mark_price.is_nan());
        assert!(ticker.oracle_price.is_nan());
    }

    #[test]
    fn test_ticker_deserialize_with_values() {
        let json = r#"{
            "symbol": "ETH-USD",
            "priceChange": 10.5,
            "priceChangePercent": 0.33,
            "lastPrice": 3200.0,
            "highPrice": 3250.0,
            "lowPrice": 3150.0,
            "volume": 1234.56,
            "quoteVolume": 3950000.0,
            "markPrice": 3201.5,
            "oraclePrice": 3200.8,
            "openInterest": 50000.0,
            "fundingRate": 0.0001
        }"#;

        let ticker: Ticker = serde_json::from_str(json).unwrap();

        assert_eq!(ticker.symbol, "ETH-USD");
        assert!((ticker.mark_price - 3201.5).abs() < 1e-6);
        assert!((ticker.oracle_price - 3200.8).abs() < 1e-6);
        assert!(!ticker.mark_price.is_nan());
        assert!(!ticker.oracle_price.is_nan());
    }

    #[test]
    fn test_l2_snapshot_deserialize() {
        let json = serde_json::json!({
            "timestamp": 1770906894450242133u64,
            "symbol": "ETH-USD",
            "updateType": "snapshot",
            "levels": [
                [
                    {"px": 1988.36, "sz": 50.0000001, "n": 5},
                    {"px": 1988.35, "sz": 71.3240001, "n": 5},
                    {"px": 1988.34, "sz": 40.00000006, "n": 4},
                    {"px": 1988.32, "sz": 102.8540001, "n": 5},
                    {"px": 1988.31, "sz": 30.00000003, "n": 3},
                    {"px": 1988.30, "sz": 30.00000003, "n": 3},
                    {"px": 1988.29, "sz": 50.8950001, "n": 5},
                    {"px": 1988.28, "sz": 64.4280001, "n": 5},
                    {"px": 1988.27, "sz": 53.7410001, "n": 5},
                    {"px": 1988.25, "sz": 121.9040001, "n": 5}
                ],
                [
                    {"px": 1988.47, "sz": 10.0, "n": 1},
                    {"px": 1988.75, "sz": 70.0, "n": 7},
                    {"px": 1989.00, "sz": 230.00000006, "n": 23},
                    {"px": 1989.25, "sz": 140.0, "n": 14},
                    {"px": 1989.50, "sz": 440.25680003, "n": 44},
                    {"px": 1989.75, "sz": 230.00000007, "n": 23},
                    {"px": 1990.00, "sz": 200.0, "n": 20},
                    {"px": 1990.25, "sz": 230.00000009, "n": 23},
                    {"px": 1990.50, "sz": 340.0, "n": 34},
                    {"px": 1990.75, "sz": 250.0, "n": 25}
                ]
            ]
        });

        let snap: L2Snapshot = serde_json::from_value(json).expect("deserialize L2Snapshot");

        assert_eq!(snap.symbol, "ETH-USD");
        assert_eq!(snap.timestamp, 1770906894450242133);

        let (bids, asks) = &snap.levels;
        assert_eq!(bids.len(), 10);
        assert_eq!(asks.len(), 10);

        // Best bid
        assert_eq!(bids[0].price, 1988.36);
        assert_eq!(bids[0].size, 50.0000001);
        assert_eq!(bids[0].num_orders, 5);

        // Best ask
        assert_eq!(asks[0].price, 1988.47);
        assert_eq!(asks[0].size, 10.0);
        assert_eq!(asks[0].num_orders, 1);

        // Last bid
        assert_eq!(bids[9].price, 1988.25);
        assert_eq!(bids[9].size, 121.9040001);
        assert_eq!(bids[9].num_orders, 5);

        // Last ask
        assert_eq!(asks[9].price, 1990.75);
        assert_eq!(asks[9].size, 250.0);
        assert_eq!(asks[9].num_orders, 25);
    }
}


