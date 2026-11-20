//! Execution reports received from external trading venues.

use crate::{common::side::Side, transaction::ActionMeta};
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use std::sync::Arc;

/// Report of an order execution at an external venue.
///
/// An external router supplies the venue and symbol as names because its instrument namespace
/// may differ from the SDK's. `order_id` correlates the execution with the originating external
/// order request. Amount, price, and USD fees are decimal values rather than fixed-point atoms.
/// The executor publishes this report as a normal `Fill`: `account` is taker, maker identifiers
/// are default values, and all fees belong to taker. Publication does not post internal trades
/// or fees. Only a configured router may submit the transaction, and it may report for another
/// account.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalFill {
    /// Execution timestamp in nanoseconds since the Unix epoch.
    pub stamp: u64,
    /// Account associated with the external execution report.
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub account: Pubkey,
    /// External venue at which the order executed.
    pub venue: Arc<str>,
    /// Identifier of the external order that produced this fill.
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id: Hash,
    /// Buy or sell direction from the reported account's perspective.
    #[serde(with = "crate::common::side::sdk_wire")]
    pub side: Side,
    /// External instrument ID, such as `FX-BTC-USD-EXT` for spot or `FXP-BTC-USD-EXT`
    /// for a perpetual, where `EXT` denotes a generic external venue.
    /// When mapping `ExternalFill` to `Fill`, a spot ID such as `FX-BTC-USD-EXT`
    /// resolves to the underlying asset `BTC`.
    pub symbol: Arc<str>,
    /// Executed quantity in the instrument's base units.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub amount: f64,
    /// Execution price in quote currency per base unit.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub price: f64,
    /// Fee amount reported by the venue, denominated in USD.
    #[serde(with = "crate::msgs::sdk_safe_f64")]
    pub fees: f64,
    /// Runtime transaction metadata; excluded from the signed payload.
    #[serde(skip)]
    pub meta: ActionMeta,
}
