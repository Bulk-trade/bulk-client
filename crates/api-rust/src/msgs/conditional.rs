use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::transaction::{Action, ActionMeta};

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
    #[serde(rename = "lim", with = "crate::msgs::fixed_point", default = "default_limit")]
    pub limit: f64,

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
    #[serde(rename = "lmin", with = "crate::msgs::fixed_point", default = "default_limit")]
    pub limit_min: f64,

    /// Limit price for low trigger (or none)
    #[serde(rename = "lmax", with = "crate::msgs::fixed_point", default = "default_limit")]
    pub limit_max: f64,

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

fn default_limit() -> f64 {
    f64::NAN
}
