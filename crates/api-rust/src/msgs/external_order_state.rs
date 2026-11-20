//! Order lifecycle reports supplied by an authorized external router.

use crate::{
    common::{order_status::OrderStatus, side::Side},
    transaction::ActionMeta,
};
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use std::sync::Arc;

/// An external order's current state, submitted by a configured router for publication.
///
/// The executor publishes a normal `OrderState` without modifying its internal order books.
/// Spot symbols resolve to their primary currency; other instruments retain their market ID.
/// Decimal amounts and prices use the instrument's base and quote units, respectively.
/// Published states use IOC and limit-order semantics, copy `amount` into `amount_original`,
/// and set maker and reduce-only flags to false.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalOrderState {
    /// Report timestamp in nanoseconds since the Unix epoch.
    pub stamp: u64,
    /// Account owning the external order, which may differ from the submitting router.
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub account: Pubkey,
    /// Identifier correlating this report with the external order request.
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id: Hash,
    /// Buy or sell direction from the order owner's perspective.
    #[serde(with = "crate::common::side::sdk_wire")]
    pub side: Side,
    /// External instrument ID, for example `FX-BTC-USD-EXT` or `FXP-BTC-USD-EXT`.
    pub symbol: Arc<str>,
    /// Current order price in quote currency per base unit.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub price: f64,
    /// Volume-weighted average execution price; zero when no quantity has executed.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub vwap: f64,
    /// Total order quantity in base units, including the completed quantity.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub amount: f64,
    /// Cumulative executed quantity in base units.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub amount_done: f64,
    /// Current lifecycle status reported by the router.
    pub status: OrderStatus,
    /// Optional explanation, such as a cancellation or rejection reason.
    pub reason: Option<String>,
    /// Runtime transaction metadata, excluded from the signed payload.
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Outbound request received on the global `orderRequests` WebSocket stream.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRequest {
    pub stamp: u64,
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id: Hash,
    pub symbol: String,
    #[serde(with = "crate::common::side::sdk_wire")]
    pub side: Side,
    pub price: f64,
    pub size: f64,
}
