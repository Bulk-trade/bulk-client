use crate::common::tif::TimeInForce;
use crate::transaction::ActionMeta;
use serde::ser::{SerializeStruct, SerializeTuple};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use std::sync::Arc;

struct FixedF64(f64);

impl Serialize for FixedF64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::msgs::fixed_point::serialize(&self.0, serializer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commission {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub to: Pubkey,
    pub fee: u8,
}

// ─────────────────────────────────────────────────────────────────────────────
// Market Order
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize)]
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

    #[serde(default)]
    pub commission: Option<Commission>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl Serialize for MarketOrder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut state = serializer
                .serialize_struct("MarketOrder", 5 + usize::from(self.commission.is_some()))?;
            state.serialize_field("c", &self.symbol)?;
            state.serialize_field("b", &self.is_buy)?;
            state.serialize_field("sz", &FixedF64(self.size))?;
            state.serialize_field("r", &self.reduce_only)?;
            state.serialize_field("i", &self.iso)?;
            if let Some(commission) = &self.commission {
                state.serialize_field("commission", commission)?;
            }
            state.end()
        } else {
            let mut tuple = serializer.serialize_tuple(6)?;
            tuple.serialize_element(&self.symbol)?;
            tuple.serialize_element(&self.is_buy)?;
            tuple.serialize_element(&FixedF64(self.size))?;
            tuple.serialize_element(&self.reduce_only)?;
            tuple.serialize_element(&self.iso)?;
            tuple.serialize_element(&self.commission)?;
            tuple.end()
        }
    }
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

#[derive(Clone, Debug, Deserialize)]
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

    #[serde(default)]
    pub commission: Option<Commission>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl Serialize for LimitOrder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut state = serializer
                .serialize_struct("LimitOrder", 7 + usize::from(self.commission.is_some()))?;
            state.serialize_field("c", &self.symbol)?;
            state.serialize_field("b", &self.is_buy)?;
            state.serialize_field("px", &FixedF64(self.price))?;
            state.serialize_field("sz", &FixedF64(self.size))?;
            state.serialize_field("tif", &self.tif)?;
            state.serialize_field("r", &self.reduce_only)?;
            state.serialize_field("i", &self.iso)?;
            if let Some(commission) = &self.commission {
                state.serialize_field("commission", commission)?;
            }
            state.end()
        } else {
            let mut tuple = serializer.serialize_tuple(8)?;
            tuple.serialize_element(&self.symbol)?;
            tuple.serialize_element(&self.is_buy)?;
            tuple.serialize_element(&FixedF64(self.price))?;
            tuple.serialize_element(&FixedF64(self.size))?;
            tuple.serialize_element(&self.tif)?;
            tuple.serialize_element(&self.reduce_only)?;
            tuple.serialize_element(&self.iso)?;
            tuple.serialize_element(&self.commission)?;
            tuple.end()
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_order_without_commission_omits_json_field() {
        assert!(!serde_json::to_value(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100.0,
            size: 1.0,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            commission: None,
            meta: ActionMeta::default(),
        })
        .expect("limit order should serialize")
        .as_object()
        .expect("limit order json should be an object")
        .contains_key("commission"));
    }

    #[test]
    fn limit_order_with_commission_includes_json_field() {
        assert_eq!(
            serde_json::to_value(LimitOrder {
                symbol: Arc::from("BTC-USD"),
                is_buy: true,
                price: 100.0,
                size: 1.0,
                tif: TimeInForce::GTC,
                reduce_only: false,
                iso: false,
                commission: Some(Commission {
                    to: Pubkey::new_unique(),
                    fee: 5,
                }),
                meta: ActionMeta::default(),
            })
            .expect("limit order should serialize")["commission"]["fee"],
            5
        );
    }

    #[test]
    fn market_order_without_commission_omits_json_field() {
        assert!(!serde_json::to_value(MarketOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: false,
            size: 1.0,
            reduce_only: false,
            iso: false,
            commission: None,
            meta: ActionMeta::default(),
        })
        .expect("market order should serialize")
        .as_object()
        .expect("market order json should be an object")
        .contains_key("commission"));
    }
}
