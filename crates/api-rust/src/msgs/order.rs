use solana_hash::Hash;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use solana_pubkey::Pubkey;
use crate::common::tif::TimeInForce;
use crate::transaction::ActionMeta;
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

    #[serde(rename = "i", default)]
    pub iso: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl MarketOrder {
    /// Compute order ID
    ///
    /// # Arguments
    /// - `account`: account associated with order
    /// - `nonce`: nonce associated with tx
    /// - `seqno`: action sequence number
    pub fn order_id(&self, account: Pubkey, nonce: u64, seqno: u32) -> Hash {
        let mut bin = Vec::<u8>::new();
        bin.extend(seqno.to_le_bytes());
        bin.extend(bincode::serialize(&self).unwrap());
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

    #[serde(rename = "i", default)]
    pub iso: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl LimitOrder {
    /// Compute order ID
    ///
    /// # Arguments
    /// - `account`: account associated with order
    /// - `nonce`: nonce associated with tx
    /// - `seqno`: action sequence number
    pub fn order_id(&self, account: Pubkey, nonce: u64, seqno: u32) -> Hash {
        let mut bin = Vec::<u8>::new();
        bin.extend(seqno.to_le_bytes());
        bin.extend(bincode::serialize(&self).unwrap());
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

    #[serde(skip)]
    pub meta: ActionMeta,
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

    #[serde(skip)]
    pub meta: ActionMeta,
}

// ─────────────────────────────────────────────────────────────────────────────
// Cancel All Orders
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelAll {
    #[serde(rename = "c")]
    pub symbols: Vec<String>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

