use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};

/// Risk policy used by the executor to recalibrate all installed risk matrices.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskConfigChange {
    /// Settlement currency used for collateral and risk calculations.
    pub settle_ccy: String,
    /// Maximum maintenance margin consumed, in percentage points in `[0, 100]`.
    /// For example, `20.0` permits consumption of 20% of maintenance margin.
    #[serde(deserialize_with = "deserialize_maxdeficit")]
    pub maxdeficit: f64,
    /// Positive multiplier on minimum liquidation volume per minute; defaults to 1.5.
    #[serde(
        default = "default_volume_scaler",
        deserialize_with = "deserialize_volume_scaler"
    )]
    pub volume_scaler: f64,
    /// Collateral margin buffer, where `0.05` is 5%.
    pub margin_buffer: f64,
    /// Multiplicative correlation adjustment in `[0, 1]`.
    pub corr_discount: f64,
    /// Fraction of positive incremental cascade premium included above raw sweep cost.
    #[serde(default = "default_cascade_factor")]
    pub cascade_factor: f64,
    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_volume_scaler() -> f64 {
    1.5
}
fn default_cascade_factor() -> f64 {
    0.25
}

/// Reject invalid MM-consumption ceilings when parsing JSON, JSON5, or bincode.
fn deserialize_maxdeficit<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || !(0.0..=100.0).contains(&value) {
        return Err(serde::de::Error::custom(
            "maxdeficit must be finite and in [0, 100]",
        ));
    }
    Ok(value)
}

/// Reject invalid execution-volume multipliers before constructing an action.
fn deserialize_volume_scaler<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if !value.is_finite() || value <= 0.0 {
        return Err(serde::de::Error::custom(
            "volume_scaler must be finite and positive",
        ));
    }
    Ok(value)
}
