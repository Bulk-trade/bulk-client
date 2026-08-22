use bulk_client::msgs::{MarketAction, OracleSource};
use bulk_sdk_core::markets::MktId;
use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Args, Debug)]
pub struct CorrsArgs {
    /// Path to correlation json5: either {index,matrix} or {matrix:{index,matrix}}.
    pub json: String,
}

#[derive(Args, Debug)]
pub struct AddMarketArgs {
    /// Market symbol, for example MINIMAX-USD.
    pub symbol: String,
}

#[derive(Args, Debug)]
pub struct ConfigSecurityArgs {
    /// Inline JSON/JSON5 or a path containing one complete security definition.
    pub json: String,
}

#[derive(Args, Debug)]
pub struct ConfigFeesArgs {
    /// Inline JSON/JSON5 or a path containing a fee-policy update.
    pub json: String,
}

#[derive(Args, Debug)]
pub struct ConfigMakerArgs {
    /// Inline JSON/JSON5 or a path containing a maker rebate tier override.
    pub json: String,
}

/// One rolling-volume fee tier.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeTierRow {
    /// Inclusive rolling-volume threshold in USD.
    pub threshold_volume: f64,
    /// Maker charge in basis points.
    pub maker_bps: f64,
    /// Taker charge in basis points.
    pub taker_bps: f64,
}

/// One maker-share rebate tier.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MakerShareTierRow {
    /// Inclusive maker-share threshold in parts per million.
    pub threshold_ppm: u32,
    /// Additional maker rebate in basis points.
    pub rebate_bps: f64,
}

/// Trading fee schedule selected by rolling volume and maker share.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeePolicy {
    /// Rolling volume window in days; currently required to be 14.
    pub window_days: u16,
    /// Volume tiers, sorted by the executor before activation.
    pub tiers: Vec<FeeTierRow>,
    /// Optional maker-share rebate tiers.
    #[serde(default)]
    pub maker_share_tiers: Vec<MakerShareTierRow>,
}

/// An immediate or scheduled fee-policy configuration update.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeePolicyUpdate {
    /// Omit for the global policy or provide a market for an override.
    #[serde(default)]
    pub instrument: Option<MktId>,
    /// Omit to apply immediately or provide the activation slot.
    pub effective_slot: Option<u64>,
    /// Clear all queued updates before applying this command.
    pub clear_scheduled: bool,
    /// Disable fee policy and return to legacy per-account settings.
    pub disable: bool,
    /// Required unless `disable` is true.
    pub policy: Option<FeePolicy>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum MarketActionArg {
    Open,
    Suspend,
    Close,
}

impl From<MarketActionArg> for MarketAction {
    fn from(action: MarketActionArg) -> Self {
        match action {
            MarketActionArg::Open => Self::Open,
            MarketActionArg::Suspend => Self::Suspend,
            MarketActionArg::Close => Self::Close,
        }
    }
}

#[derive(Args, Debug)]
pub struct MarketAdminArgs {
    /// Market symbol, for example BTC-USD.
    pub symbol: String,
    /// Administrative market-state transition.
    #[arg(value_enum)]
    pub action: MarketActionArg,
    /// Optional synthetic close price. Valid only with `close`.
    #[arg(long)]
    pub price: Option<f64>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OracleSourceArg {
    Both,
    Pyth,
    Bulk,
}

impl From<OracleSourceArg> for OracleSource {
    fn from(source: OracleSourceArg) -> Self {
        match source {
            OracleSourceArg::Both => Self::Both,
            OracleSourceArg::Pyth => Self::Pyth,
            OracleSourceArg::Bulk => Self::Bulk,
        }
    }
}

#[derive(Args, Debug)]
pub struct PricingAdminArgs {
    /// Oracle instrument, for example BTC.
    pub instrument: String,
    /// Accepted publisher source.
    #[arg(value_enum)]
    pub source: OracleSourceArg,
}
