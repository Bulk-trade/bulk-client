use std::fmt::Write;
use std::fmt;
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use crate::common::{write_bool, write_f64, write_pubkey_bytes, write_string_u32, write_string_u64, write_u32, write_u64, write_u8};
use crate::common::side::Side;
use crate::common::tif::TimeInForce;
use crate::msgs::DECIMALS_MULTIPLIER;

/// A limit order.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct LimitOrder {
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
    pub reduce_only: bool,
    pub time_in_force: TimeInForce,
    /// Explicit nonce (micro-timestamp). Required for order-ID generation.
    pub nonce: Option<u64>,
    /// Pre-computed order ID (base58). If `None`, call [`order_id`].
    pub oid: Option<String>,
    /// Account public key (base58). Required for order-ID generation.
    pub pubkey: Option<Pubkey>,
}

#[allow(unused)]
impl LimitOrder {
    pub fn new(
        symbol: impl Into<String>,
        side: Side,
        price: f64,
        size: f64,
        tif: TimeInForce,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            price,
            size,
            reduce_only: false,
            time_in_force: tif,
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
            let pubkey = self.pubkey
                .ok_or_else(|| eyre::eyre!("order_id: pubkey missing"))?;

            // Serialize with u32 string-length prefix (order-ID format)
            let mut buf = Vec::with_capacity(128);
            write_u64(&mut buf, nonce);
            write_string_u32(&mut buf, &self.symbol);
            write_pubkey_bytes(&mut buf, &pubkey);
            write_u8(&mut buf, self.side.into());
            write_u64(&mut buf, (self.size * DECIMALS_MULTIPLIER).round() as u64);
            write_u64(&mut buf, (self.price * DECIMALS_MULTIPLIER).round() as u64);
            write_u32(&mut buf, self.time_in_force as u32);
            write_bool(&mut buf, self.reduce_only);

            let hash = Sha256::digest(&buf);
            let oid = bs58::encode(hash).into_string();
            self.oid = Some(oid.clone());
            Ok(oid)
        }

    }

    /// Write compact JSON directly into a buffer — no intermediate allocations.
    /// ```json
    /// {
    ///   "order": {
    ///     "c": "BTC-USD",
    ///     "b": true,
    ///     "px": 98000.0,
    ///     "sz": 1.0,
    ///     "r": false,
    ///     "t": { "limit": { "tif": "GTC" } }
    ///   }
    /// }
    /// ```
    pub fn write_api(&self, buf: &mut String) {
        let tif = match self.time_in_force {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::ALO => "ALO",
        };
        let b = self.side == Side::Buy;
        write!(
            buf,
            r#"{{"order":{{"c":"{}","b":{},"px":{},"sz":{},"r":{},"t":{{"limit":{{"tif":"{}"}}}}}}}}"#,
            self.symbol, b, self.price, self.size, self.reduce_only, tif
        ).unwrap();
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
        write_f64(buf, self.price);
        write_f64(buf, self.size);
        write_bool(buf, self.reduce_only);
        // limit variant tag = 0
        write_u32(buf, 0);
        write_u32(buf, self.time_in_force as u32);
        // no cloid
        write_bool(buf, false);
    }
}

impl fmt::Display for LimitOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LimitOrder({} {} {} @ {}", self.side, self.size, self.symbol, self.price)?;
        write!(f, ", tif={:?}", self.time_in_force)?;
        if let Some(ref oid) = self.oid {
            write!(f, ", oid={oid}")?;
        }
        write!(f, ")")
    }
}
