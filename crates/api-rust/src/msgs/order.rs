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
pub struct BuilderCode {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub to: Pubkey,
    pub fee: u8,
}

fn deserialize_builder_code<'de, D>(deserializer: D) -> Result<Option<BuilderCode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return <Option<BuilderCode> as Deserialize>::deserialize(deserializer);
    }

    struct BuilderCodeVisitor;

    impl<'de> serde::de::Visitor<'de> for BuilderCodeVisitor {
        type Value = Option<BuilderCode>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an omitted builderCode field or a builderCode object")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("builderCode must be omitted or an object"))
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Err(E::custom("builderCode must be omitted or an object"))
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            <BuilderCode as Deserialize>::deserialize(deserializer).map(Some)
        }
    }

    deserializer.deserialize_option(BuilderCodeVisitor)
}

struct OrderHashSafeF64(f64);

impl Serialize for OrderHashSafeF64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::msgs::fixed_point::serialize(&self.0, serializer)
    }
}

struct OrderHashMarketOrder<'a>(&'a MarketOrder);

impl Serialize for OrderHashMarketOrder<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(5)?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        tuple.end()
    }
}

struct OrderHashLimitOrder<'a>(&'a LimitOrder);

impl Serialize for OrderHashLimitOrder<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(7)?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.price))?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.tif)?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        tuple.end()
    }
}

struct OrderHashMarketAction<'a>(&'a MarketOrder);

impl Serialize for OrderHashMarketAction<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_variant(
            "Action",
            0,
            "MarketOrder",
            &OrderHashMarketOrder(self.0),
        )
    }
}

struct OrderHashLimitAction<'a>(&'a LimitOrder);

impl Serialize for OrderHashLimitAction<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_variant(
            "Action",
            1,
            "LimitOrder",
            &OrderHashLimitOrder(self.0),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Market Order
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

    #[serde(
        rename = "builderCode",
        default,
        deserialize_with = "deserialize_builder_code"
    )]
    pub builder_code: Option<BuilderCode>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl Serialize for MarketOrder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut state = serializer
                .serialize_struct("MarketOrder", 5 + usize::from(self.builder_code.is_some()))?;
            state.serialize_field("c", &self.symbol)?;
            state.serialize_field("b", &self.is_buy)?;
            state.serialize_field("sz", &FixedF64(self.size))?;
            state.serialize_field("r", &self.reduce_only)?;
            state.serialize_field("i", &self.iso)?;
            if let Some(commission) = &self.builder_code {
                state.serialize_field("builderCode", commission)?;
            }
            state.end()
        } else {
            let mut tuple = serializer.serialize_tuple(6)?;
            tuple.serialize_element(&self.symbol)?;
            tuple.serialize_element(&self.is_buy)?;
            tuple.serialize_element(&FixedF64(self.size))?;
            tuple.serialize_element(&self.reduce_only)?;
            tuple.serialize_element(&self.iso)?;
            tuple.serialize_element(&self.builder_code)?;
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
        bin.extend(bincode::serialize(&OrderHashMarketAction(self)).unwrap());
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
#[serde(deny_unknown_fields)]
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

    #[serde(
        rename = "builderCode",
        default,
        deserialize_with = "deserialize_builder_code"
    )]
    pub builder_code: Option<BuilderCode>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

impl Serialize for LimitOrder {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            let mut state = serializer
                .serialize_struct("LimitOrder", 7 + usize::from(self.builder_code.is_some()))?;
            state.serialize_field("c", &self.symbol)?;
            state.serialize_field("b", &self.is_buy)?;
            state.serialize_field("px", &FixedF64(self.price))?;
            state.serialize_field("sz", &FixedF64(self.size))?;
            state.serialize_field("tif", &self.tif)?;
            state.serialize_field("r", &self.reduce_only)?;
            state.serialize_field("i", &self.iso)?;
            if let Some(commission) = &self.builder_code {
                state.serialize_field("builderCode", commission)?;
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
            tuple.serialize_element(&self.builder_code)?;
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
        bin.extend(bincode::serialize(&OrderHashLimitAction(self)).unwrap());
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
    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
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
    fn limit_order_without_builder_code_omits_json_field() {
        assert!(!serde_json::to_value(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100.0,
            size: 1.0,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta::default(),
        })
        .expect("limit order should serialize")
        .as_object()
        .expect("limit order json should be an object")
        .contains_key("builderCode"));
    }

    #[test]
    fn limit_order_with_builder_code_includes_json_field() {
        assert_eq!(
            serde_json::to_value(LimitOrder {
                symbol: Arc::from("BTC-USD"),
                is_buy: true,
                price: 100.0,
                size: 1.0,
                tif: TimeInForce::GTC,
                reduce_only: false,
                iso: false,
                builder_code: Some(BuilderCode {
                    to: Pubkey::new_unique(),
                    fee: 5,
                }),
                meta: ActionMeta::default(),
            })
            .expect("limit order should serialize")["builderCode"]["fee"],
            5
        );
    }

    #[test]
    fn market_order_without_builder_code_omits_json_field() {
        assert!(!serde_json::to_value(MarketOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: false,
            size: 1.0,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta::default(),
        })
        .expect("market order should serialize")
        .as_object()
        .expect("market order json should be an object")
        .contains_key("builderCode"));
    }

    #[test]
    fn order_json_rejects_null_builder_code() {
        assert!(serde_json::from_value::<LimitOrder>(serde_json::json!({
            "c": "BTC-USD",
            "b": true,
            "px": 100.0,
            "sz": 1.0,
            "tif": "GTC",
            "r": false,
            "i": false,
            "builderCode": null
        }))
        .is_err());

        assert!(serde_json::from_value::<MarketOrder>(serde_json::json!({
            "c": "BTC-USD",
            "b": true,
            "sz": 1.0,
            "r": false,
            "i": false,
            "builderCode": null
        }))
        .is_err());
    }

    #[test]
    fn order_id_ignores_commission() {
        let account = Pubkey::new_unique();
        let to = Pubkey::new_unique();

        let without = LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100.0,
            size: 1.0,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta::default(),
        };
        let with = LimitOrder {
            builder_code: Some(BuilderCode { to, fee: 5 }),
            ..without.clone()
        };

        assert_eq!(
            without.order_id(account, 7, 3),
            with.order_id(account, 7, 3)
        );

        let market_without = MarketOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: false,
            size: 2.0,
            reduce_only: false,
            iso: true,
            builder_code: None,
            meta: ActionMeta::default(),
        };
        let market_with = MarketOrder {
            builder_code: Some(BuilderCode { to, fee: 15 }),
            ..market_without.clone()
        };

        assert_eq!(
            market_without.order_id(account, 7, 3),
            market_with.order_id(account, 7, 3)
        );
    }
}
