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
    // Settlement currency
    pub settle_ccy: String,
    // Maximum expected dollar loss below collateral allowed (for example 15k$)
    pub max_loss: f64,
    // Expected loss floor in bps (for example 500bps)
    pub eloss_floor: f64,
    // Maximum p(liquidation) allowed (for example 90%)
    pub max_pliq: f64,
    // margin buffer (5% = 0.05)
    pub margin_buffer: f64,
    // correlation discount [0-1], lower value reduces portfolio correlations -> higher risk
    pub corr_discount: f64,

    #[serde(skip)]
    pub meta: ActionMeta,
}
