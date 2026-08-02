use bulk_client::msgs::{MarketAction, OracleSource};
use clap::{Args, ValueEnum};

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
