use std::fmt;
use crate::common::{write_f64, write_string_u64, write_u64};


/// A market order.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct OraclePrice {
    pub timestamp: u64,
    pub symbol: String,
    pub price: f64,
}

#[allow(unused)]
impl OraclePrice {
    pub fn new(
        timestamp: u64,
        symbol: impl Into<String>,
        price: f64,
    ) -> Self {
        Self {
            timestamp,
            symbol: symbol.into(),
            price
        }
    }

    /// Produce the compact JSON payload expected by the exchange API.
    pub fn to_api(&self) -> serde_json::Value {
        serde_json::json!({
            "t": self.timestamp,
            "c": self.symbol,
            "px": self.price,
        })
    }

    /// Serialize for inclusion in a **transaction** (signing context).
    ///
    /// Uses the `u64` string-length prefix convention.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) {
        write_u64(buf, self.timestamp);
        write_string_u64(buf, &self.symbol);
        write_f64(buf, self.price);
    }
}

impl fmt::Display for OraclePrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OraclePrice({} = {})", self.symbol, self.price)
    }
}
