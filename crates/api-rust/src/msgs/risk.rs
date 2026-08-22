use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};

/// Risk configuration
/// - target maximum dollar loss
/// - minimum eloss bps
/// - maximum p(liquidation) allowed
///
/// We determine the E[loss] threshold as:
/// ```
/// let max_loss = 15_000.0;
/// let notional = 1_000_000.0;
/// let eloss_floor = 500.0;
/// let eloss_notional = max_loss / notional * 1e4;
/// let eloss_max = (4.0 * max_loss) / notional * 1e4;
/// let eloss_target = f64::max(eloss_notional, eloss_floor).min(eloss_max);
/// assert_eq!(eloss_target, 500.0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RiskConfigChange {
    /// Settlement currency used for collateral and risk calculations.
    pub settle_ccy: String,
    /// Maximum expected dollar loss below collateral allowed (for example $15k).
    pub max_loss: f64,
    /// Expected-loss floor in basis points (for example 500 bps).
    pub eloss_floor: f64,
    /// Maximum permitted probability of liquidation (for example 90%).
    pub max_pliq: f64,
    /// Collateral margin buffer, where `0.05` is 5%.
    pub margin_buffer: f64,
    /// Correlation discount in `[0, 1]`; lower values increase portfolio risk.
    pub corr_discount: f64,
    /// Fraction of positive incremental cascade premium included above raw sweep cost.
    #[serde(default = "default_cascade_factor")]
    pub cascade_factor: f64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_cascade_factor() -> f64 {
    0.25
}
