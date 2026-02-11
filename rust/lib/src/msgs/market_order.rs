use std::fmt;
use sha2::{Digest, Sha256};
use bulk_sdk_core::Side;
use bulk_sdk_core::trade::TimeInForce;
use crate::common::{write_bool, write_f64, write_pubkey, write_string_u32, write_string_u64, write_u32, write_u64, write_u8};

/// 8-decimal fixed-point multiplier used for order-ID hashing.
const DECIMALS_MULTIPLIER: f64 = 100_000_000.0;


/// A market order.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct MarketOrder {
    pub symbol: String,
    pub side: Side,
    pub size: f64,
    pub reduce_only: bool,
    /// Explicit nonce (micro-timestamp). Required for order-ID generation.
    pub nonce: Option<u64>,
    /// Pre-computed order ID (base58). If `None`, call [`order_id`].
    pub oid: Option<String>,
    /// Account public key (base58). Required for order-ID generation.
    pub pubkey: Option<String>,
}

impl MarketOrder {
    pub fn new(
        symbol: impl Into<String>,
        side: Side,
        size: f64,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            size,
            reduce_only: false,
            nonce: None,
            oid: None,
            pubkey: None,
        }
    }

    /// Deterministic order ID = base58(sha256(serialized fields)).
    ///
    /// Requires either a pre-set `oid`, or both `nonce` and `pubkey`.
    pub fn order_id(&mut self) -> eyre::Result<String> {
        if let Some(ref oid) = self.oid {
            Ok(oid.clone())
        } else {
            let nonce = self.nonce
                .ok_or_else(|| eyre::eyre!("order_id: nonce missing"))?;
            let pubkey = self.pubkey.as_deref()
                .ok_or_else(|| eyre::eyre!("order_id: pubkey missing"))?;

            // Serialize with u32 string-length prefix (order-ID format)
            let mut buf = Vec::with_capacity(128);
            write_u64(&mut buf, nonce);
            write_string_u32(&mut buf, &self.symbol);
            write_pubkey(&mut buf, pubkey)?;
            write_u8(&mut buf, self.side.into());
            write_u64(&mut buf, (self.size * DECIMALS_MULTIPLIER).round() as u64);
            write_bool(&mut buf, self.reduce_only);

            let hash = Sha256::digest(&buf);
            let oid = bs58::encode(hash).into_string();
            self.oid = Some(oid.clone());
            Ok(oid)
        }

    }

    /// Produce the compact JSON payload expected by the exchange API.
    ///
    /// ```json
    /// {
    ///   "order": {
    ///     "c": "BTC-USD",
    ///     "b": true,
    ///     "sz": 1.0,
    ///     "r": false,
    ///     "t": "trigger": {
    ///       "is_market": true,
    ///       "triggerPx": 0.0
    ///     }
    ///   }
    /// }
    /// ```
    pub fn to_api(&self) -> serde_json::Value {
        serde_json::json!({
            "order": {
                "c": self.symbol,
                "b": self.side == Side::Buy,
                "sz": self.size,
                "r": self.reduce_only,
                "t": {
                    "trigger": {
                        "is_market": true,
                        "triggerPx": 0.0
                    }
                }
            }
        })
    }

    /// Serialize for inclusion in a **transaction** (signing context).
    ///
    /// Uses the `u64` string-length prefix convention.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) {
        // order type tag = 0 (order)
        write_u32(buf, 0);
        // fields
        write_string_u64(buf, &self.symbol);
        write_bool(buf, self.side == Side::Buy);
        write_f64(buf, self.size);
        write_bool(buf, self.reduce_only);
        // market variant tag = 0
        write_u32(buf, 1);
        write_f64(buf, 0.0);
        // no cloid
        write_bool(buf, false);
    }
}

impl fmt::Display for MarketOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MarketOrder({} {} {} @ <any>", self.side, self.size, self.symbol)?;
        if let Some(ref oid) = self.oid {
            write!(f, ", oid={oid}")?;
        }
        write!(f, ")")
    }
}
