use std::fmt;
use crate::common::{write_string_u64, write_u32};


/// Cancel order.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct CancelOrder {
    pub symbol: String,
    pub oid: String,
}

#[allow(unused)]
impl CancelOrder {
    pub fn new(
        symbol: impl Into<String>,
        oid: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            oid: oid.into(),
        }
    }

    /// Produce the compact JSON payload expected by the exchange API.
    pub fn to_api(&self) -> serde_json::Value {
        serde_json::json!({
            "cancel": {
                "c": self.symbol,
                "oid": self.oid
            }
        })
    }

    /// Serialize for inclusion in a **transaction** (signing context).
    ///
    /// Uses the `u64` string-length prefix convention.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) -> eyre::Result<()> {
        // order type tag = 1 (cancel)
        write_u32(buf, 1);
        // fields
        write_string_u64(buf, &self.symbol);
        // oid is base58-encoded hash → decode to raw bytes
        let oid_bytes = bs58::decode(&self.oid).into_vec()?;
        buf.extend_from_slice(&oid_bytes);
        Ok(())
    }
}

impl fmt::Display for CancelOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelOrder({}, oid={})", self.symbol, self.oid)
    }
}
