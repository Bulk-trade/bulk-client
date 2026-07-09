use crate::transaction::{Action, ActionMeta};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Information for either a Stop or Take-Profit Order
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StopOrTP {
    /// Which Instrument
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    /// Indicates whether above or below threshold to trigger
    #[serde(rename = "d")]
    pub is_above: bool,

    /// Size to be done if triggered
    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
    pub size: f64,

    /// Trigger threshold
    #[serde(rename = "tr", with = "crate::msgs::fixed_point")]
    pub threshold: f64,

    /// Optional limit px if will trigger a limit order
    #[serde(
        rename = "lim",
        with = "crate::msgs::opt_fixed_point",
        default = "default_limit"
    )]
    pub limit: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// A combined take-profit + stop operating in a collar
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Range {
    /// Which Instrument
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    /// Indicates whether the underlying position is buy or sell
    #[serde(rename = "d")]
    pub is_buy: bool,

    /// Size to be done if triggered
    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
    pub size: f64,

    /// Trigger threshold (low)
    #[serde(rename = "pmin", with = "crate::msgs::fixed_point")]
    pub collar_min: f64,

    /// Trigger threshold (high)
    #[serde(rename = "pmax", with = "crate::msgs::fixed_point")]
    pub collar_max: f64,

    /// Limit price for low trigger (or none)
    #[serde(
        rename = "lmin",
        with = "crate::msgs::opt_fixed_point",
        default = "default_limit"
    )]
    pub limit_min: Option<f64>,

    /// Limit price for low trigger (or none)
    #[serde(
        rename = "lmax",
        with = "crate::msgs::opt_fixed_point",
        default = "default_limit"
    )]
    pub limit_max: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Trigger evaluates a collection of actions when trigger reached
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trigger {
    /// Which Instrument
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    /// Indicates whether the trigger is above or below
    #[serde(rename = "d")]
    pub is_above: bool,

    /// Trigger threshold
    #[serde(rename = "tr", with = "crate::msgs::fixed_point")]
    pub threshold: f64,

    /// Actions to be evaluated on trigger
    pub actions: Vec<Action>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Trailing stop configuration.
///
/// The executor materializes this as a protective stop leg plus a rotating
/// sentinel leg that ratchets the stop when price moves favorably by `step_bps`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trailing {
    /// Which Instrument
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    /// Indicates whether protected position direction is buy/long.
    #[serde(rename = "b")]
    pub is_buy: bool,

    /// Size to be done if triggered
    #[serde(rename = "sz", with = "crate::msgs::fixed_point")]
    pub size: f64,

    /// Trailing distance in basis points.
    #[serde(rename = "trb")]
    pub trail_bps: u32,

    /// Favorable reset step in basis points.
    #[serde(rename = "stb")]
    pub step_bps: u32,

    /// Optional limit px if stop trigger should place a limit order.
    #[serde(
        rename = "lim",
        with = "crate::msgs::opt_fixed_point",
        default = "default_limit"
    )]
    pub limit: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// On-fill registration.
///
/// Registers follow-up actions that should execute once the parent action
/// (identified by `parent_seqno` in the same transaction) receives its first fill.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnFill {
    /// Parent action sequence index in the same transaction.
    #[serde(rename = "p")]
    pub parent_seqno: u32,

    /// Actions to execute on first parent fill.
    pub actions: Vec<Action>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_limit() -> Option<f64> {
    None
}
