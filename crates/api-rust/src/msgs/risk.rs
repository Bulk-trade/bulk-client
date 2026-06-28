use crate::msgs::serde_pubkey;
use serde::{Deserialize, Serialize};
use solana_keypair::Pubkey;
use crate::common::instrument_config::InstrumentConfig;
use crate::transaction::ActionMeta;


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
    
    #[serde(skip)]
    pub meta: ActionMeta,
}
