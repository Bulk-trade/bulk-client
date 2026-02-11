use std::fmt;
use sha2::{Digest, Sha256};
use bulk_sdk_core::Side;
use bulk_sdk_core::trade::TimeInForce;
use crate::common::{write_bool, write_f64, write_pubkey, write_string_u32, write_string_u64, write_u32, write_u64, write_u8};


/// Cancel All.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct CancelAll {
    pub symbols: Vec<String>,
}

impl CancelAll {
    pub fn new(
        symbols: Vec<String>,
    ) -> Self {
        Self {
            symbols,
        }
    }

    /// Produce the compact JSON payload expected by the exchange API.
    pub fn to_api(&self) -> serde_json::Value {
        serde_json::json!({
            "cancelAll": {
                "c": self.symbols,
            }
        })
    }

    /// Serialize for inclusion in a **transaction** (signing context).
    ///
    /// Uses the `u64` string-length prefix convention.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) -> eyre::Result<()> {
        // order type tag = 2 (cancelAll)
        write_u32(buf, 2);
        write_u64(buf, self.symbols.len() as u64);
        for sym in &self.symbols {
            write_string_u64(buf, sym);
        }
        Ok(())
    }
}

impl fmt::Display for CancelAll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CancelAll({:?})", self.symbols)
    }
}
