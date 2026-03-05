use solana_hash::Hash;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use solana_pubkey::Pubkey;
use crate::common::tif::TimeInForce;

// ─────────────────────────────────────────────────────────────────────────────
// Market Order
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketOrder {
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    #[serde(rename = "b")]
    pub is_buy: bool,

    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
    pub size: f64,

    #[serde(rename = "r")]
    pub reduce_only: bool,
}

impl MarketOrder {
    /// Compute order ID
    ///
    /// # Arguments
    /// - `account`: account associated with order
    /// - `nonce`: nonce associated with tx
    pub fn order_id(&self, account: Pubkey, nonce: u64) -> Hash {
        let mut bin = bincode::serialize(&self).unwrap();
        bin.extend_from_slice(account.as_ref());
        bin.extend_from_slice(&nonce.to_le_bytes());

        let mut hasher = sha2::Sha256::new();
        hasher.update(&bin);
        let hash: [u8; 32] = hasher.finalize().into();
        Hash::from(hash)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Limit Order
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimitOrder {
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    #[serde(rename = "b")]
    pub is_buy: bool,

    #[serde(rename = "px", with = "crate::msgs::fixed_point")]
    pub price: f64,

    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
    pub size: f64,

    #[serde(rename = "tif")]
    pub tif: TimeInForce,

    #[serde(rename = "r")]
    pub reduce_only: bool,
}

impl LimitOrder {
    /// Compute order ID
    ///
    /// # Arguments
    /// - `account`: account associated with order
    /// - `nonce`: nonce associated with tx
    pub fn order_id(&self, account: Pubkey, nonce: u64) -> Hash {
        let mut bin = bincode::serialize(&self).unwrap();
        bin.extend_from_slice(account.as_ref());
        bin.extend_from_slice(&nonce.to_le_bytes());

        let mut hasher = sha2::Sha256::new();
        hasher.update(&bin);
        let hash: [u8; 32] = hasher.finalize().into();
        Hash::from(hash)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Modify Order
// ─────────────────────────────────────────────────────────────────────────────

/// Update order: changing order size
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModifyOrder {
    #[serde(with = "crate::msgs::serde_hash", rename = "oid")]
    pub order_id: Hash,
    #[serde(rename = "c")]
    pub symbol: String,
    #[serde(rename = "sz")]
    pub amount: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cancel Order
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelOrder {
    #[serde(rename = "c")]
    pub symbol: String,
    #[serde(with = "crate::msgs::serde_hash", rename = "oid")]
    pub oid: Hash,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cancel All Orders
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelAll {
    #[serde(rename = "c")]
    pub symbols: Vec<String>,
}
