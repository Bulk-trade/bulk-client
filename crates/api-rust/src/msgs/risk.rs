use crate::msgs::serde_pubkey;
use serde::{Deserialize, Serialize};
use solana_keypair::Pubkey;
use crate::common::instrument_config::InstrumentConfig;
use crate::transaction::ActionMeta;

/// Liquidation configuration
/// - liquidation behavior parameters
/// - insurance fund parameters
///
/// # ADL ranking
/// For any given asset, all accounts with profitable position are ranked as follows:
/// ```not_run
///     score = pnl^w x leverage^(1-w)
/// ```
/// Any ADL is then applied pro-rata according to the score
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LiquidationConfig {
    // settlement ccy
    pub settlement_ccy: String,
    // ADL bias [0, 1], where bias > 0.5 provides more weight to P&L over leverage
    pub adl_bias: f64,

    // maximum depth to sweep (bps)
    pub maxdepth: f64,
    // minimum number of levels to leave on book when sweeping
    pub sweep_residual: usize,
}

/// Risk Vault configuration
/// - % of ADL that hits the vault
/// - depth where we introduce liquidity
/// - vault key
/// - ...
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskVaultConfig {
    #[serde(with = "serde_pubkey")]
    pub owner: Pubkey,
    // allocation per instrument
    pub allocations: InstrumentConfig<f64>,
    // default depth (bps) where vault takes over in liquidations
    pub liquidity: InstrumentConfig<f64>,
}

/// Risk configuration
/// - target maximum dollar loss
/// - minimum eloss bps
/// - maximum p(liquidation) allowed
///
/// We determine the E[loss] threshold as:
/// ```
///    let eloss_notional = max_loss / notional * 1e4;
///    let eloss_max = (4.0 * max_loss) / notional * 1e4;
///    let eloss_target = f64::max(eloss_notional, eloss_floor).min(eloss_max)
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskConfigChange {
    // Maximum expected dollar loss below collateral allowed (for example 15k$)
    pub max_loss: f64,
    // Expected loss floor in bps (for example 500bps)
    pub eloss_floor: f64,
    // Maximum p(liquidation) allowed (for example 90%)
    pub max_pliq: f64,
    // margin buffer (5% = 0.05)
    pub margin_buffer: f64,

    // liquidation config
    pub liquidation: LiquidationConfig,
    // insurance vault config
    pub vault: RiskVaultConfig,
    
    #[serde(skip)]
    pub meta: ActionMeta,
}
