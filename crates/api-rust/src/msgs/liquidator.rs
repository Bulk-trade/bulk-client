use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::collections::HashMap;

/// Per instrument liquidation strategy config
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LiqConfigByInstrument {
    /// maximum notional exposure we are willing to take on
    pub max_exposure: f64,
    /// Minimum liquidation reserve above mid, in basis points
    pub reserve: f64,
    /// Additional execution-impact reserve factor
    pub rfactor: f64,

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
    /// configuration per instrument
    pub instrument: HashMap<String, LiqConfigByInstrument>,

    #[serde(skip)]
    pub meta: ActionMeta,
}
