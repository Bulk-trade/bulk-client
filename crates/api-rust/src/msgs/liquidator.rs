use solana_keypair::Pubkey;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::transaction::ActionMeta;

/// Per instrument liquidation strategy config
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiqConfigByInstrument {
    /// maximum notional exposure we are willing to take on
    pub max_exposure: f64,
    /// minimum premium above mid
    pub premium_min: f64,
    /// fee (bps) applied to computed liquidation price
    pub fee: f64,

    /// maximum ADL absorption
    pub max_adl_notional: f64,
    /// maximum ADL % taken
    pub max_adl_percent: f64,
}

/// Liquidation and ADL related config
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiqConfig {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub owner: Pubkey,
    /// maximum cross exposure across assets
    pub cross_exposure: f64,
    /// Scoring skew (P&L vs leverage)
    pub scoring_skew: f64,
    /// % of volume to take <- strategy specific
    pub percent_volume: f64,

    /// default instrument config
    pub defaults: LiqConfigByInstrument,
    /// configuration per instrument
    pub overrides: HashMap<String, LiqConfigByInstrument>,

    #[serde(skip)]
    pub meta: ActionMeta,
}
