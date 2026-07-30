use crate::transaction::{Action, ActionMeta};
use serde::{de, Deserialize, Deserializer, Serialize};
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

    #[serde(rename = "i", default)]
    pub iso: bool,

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

    #[serde(rename = "i", default)]
    pub iso: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Trigger evaluates a collection of actions when trigger reached
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

    #[serde(rename = "i", default)]
    pub iso: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// On-fill registration.
///
/// Registers follow-up actions that should execute once the trigger action
/// receives its first fill.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnFill {
    /// Market or limit action that serves as the trigger.
    #[serde(deserialize_with = "deserialize_on_fill_trigger")]
    pub trigger: Box<Action>,

    /// Actions to execute on first parent fill.
    pub actions: Vec<Action>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn deserialize_on_fill_trigger<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Box<Action>, D::Error> {
    let trigger = Box::<Action>::deserialize(deserializer)?;
    if matches!(
        trigger.as_ref(),
        Action::MarketOrder(_) | Action::LimitOrder(_)
    ) {
        Ok(trigger)
    } else {
        Err(de::Error::custom(
            "on-fill trigger must be a market or limit order",
        ))
    }
}

fn default_limit() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn conditional_orders_preserve_their_iso_field_but_trigger_does_not_have_one() {
        for (kind, payload) in [
            (
                "st",
                json!({"c":"BTC-USD","d":true,"sz":1.0,"tr":100.0,"lim":null,"i":true}),
            ),
            (
                "tp",
                json!({"c":"BTC-USD","d":false,"sz":1.0,"tr":100.0,"lim":null,"i":true}),
            ),
            (
                "rng",
                json!({
                    "c":"BTC-USD","d":true,"sz":1.0,"pmin":90.0,"pmax":110.0,
                    "lmin":null,"lmax":null,"i":true
                }),
            ),
            (
                "trl",
                json!({
                    "c":"BTC-USD","b":true,"sz":1.0,"trb":100,"stb":10,
                    "lim":null,"i":true
                }),
            ),
        ] {
            let action: Action =
                serde_json::from_value(json!({kind: payload})).expect("valid conditional action");
            assert_eq!(
                serde_json::to_value(action).expect("serialize conditional action")[kind]["i"],
                true
            );
        }

        let trigger: Action = serde_json::from_value(json!({
            "trig": {
                "c": "BTC-USD",
                "d": true,
                "tr": 100.0,
                "actions": []
            }
        }))
        .expect("valid trigger basket");
        assert!(
            serde_json::to_value(trigger).expect("serialize trigger")["trig"]
                .get("i")
                .is_none()
        );
    }

    #[test]
    fn trigger_rejects_a_top_level_iso_field_but_allows_it_on_nested_orders() {
        assert!(serde_json::from_value::<Action>(json!({
            "trig": {
                "c": "BTC-USD",
                "d": true,
                "tr": 100.0,
                "i": false,
                "actions": []
            }
        }))
        .is_err());

        serde_json::from_value::<Action>(json!({
            "trig": {
                "c": "BTC-USD",
                "d": true,
                "tr": 100.0,
                "actions": [{
                    "m": {
                        "c": "BTC-USD",
                        "b": true,
                        "sz": 1.0,
                        "r": false,
                        "i": true
                    }
                }]
            }
        }))
        .expect("nested order iso remains valid");
    }
}
