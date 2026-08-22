use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};

/// Per instrument liquidation strategy config
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiqConfigByInstrument {
    /// which instrument
    pub symbol: String,
    /// maximum notional exposure we are willing to take on
    pub max_exposure: f64,
    /// Minimum liquidation reserve above mid, in basis points
    pub reserve: f64,
    /// Additional execution-impact reserve factor
    pub rfactor: f64,
    /// % of volume to take <- strategy specific
    pub volume_percent: f64,
    /// target minimum volume / min
    pub volume_min: f64,
    /// volume rampup period in seconds
    pub volume_rampup: u64,
    /// Absolute sweep-cost ceiling relative to fair and oracle prices, in basis points
    #[serde(default = "default_max_sweep_bps")]
    pub max_sweep_bps: f64,
    /// maximum ADL absorption
    pub max_adl_notional: f64,
    /// maximum ADL % taken
    pub max_adl_percent: f64,
}

/// Liquidation and ADL related config
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiqConfig {
    /// maximum cross exposure across assets
    pub cross_exposure: f64,
    /// Scoring skew (P&L vs leverage)
    pub scoring_skew: f64,
    /// Toxicity downscales max-exposure (range 0 - 100)
    pub toxicity: f64,
    /// Fraction of effective max exposure at which shortfall-size urgency is fully scaled
    #[serde(default = "default_urgency_size_fraction")]
    pub urgency_size_fraction: f64,
    /// Standard deviations above expected sweep cost allowed at normal urgency
    #[serde(default = "default_sweep_sds")]
    pub sweep_sds: f64,
    /// configuration per instrument
    pub instruments: Vec<LiqConfigByInstrument>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_urgency_size_fraction() -> f64 {
    0.25
}

fn default_sweep_sds() -> f64 {
    2.0
}

fn default_max_sweep_bps() -> f64 {
    100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_fields_and_defaults_match_the_sdk() {
        let config: LiqConfig = serde_json::from_value(serde_json::json!({
            "cross_exposure": 15_000_000.0,
            "scoring_skew": 0.5,
            "toxicity": 10.0,
            "instruments": [{
                "symbol": "BTC-USD",
                "max_exposure": 12_000_000.0,
                "reserve": 75.0,
                "rfactor": 0.25,
                "volume_percent": 30.0,
                "volume_min": 1.0,
                "volume_rampup": 60,
                "max_adl_notional": 5_000_000.0,
                "max_adl_percent": 50.0
            }]
        }))
        .expect("SDK-compatible liquidator action");

        assert_eq!(config.urgency_size_fraction, 0.25);
        assert_eq!(config.sweep_sds, 2.0);
        assert_eq!(config.instruments[0].max_sweep_bps, 100.0);
    }
}
