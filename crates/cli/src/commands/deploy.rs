//! Runtime market-onboarding subcommand arguments.
use clap::Args;

/// Register securities (currency + perp). File is a json5 array; currency must come first.
#[derive(Args, Debug)]
pub struct ConfigSecurityArgs {
    /// Path to securities json5 (array of Currency/Perpetual)
    pub json: String,
}

/// Configure a per-coin model (fair-price / regime / volatility).
#[derive(Args, Debug)]
pub struct ConfigModelArgs {
    /// Coin registration key (e.g. MINIMAX, MU)
    pub coin: String,
    /// Path to the model json5
    pub json: String,
}

/// Install a coin's risk matrix from the Monte-Carlo CSV.
#[derive(Args, Debug)]
pub struct ConfigRiskMatrixArgs {
    /// Coin registration key (e.g. MINIMAX, MU)
    pub coin: String,
    /// Path to the risk-matrix CSV
    pub csv: String,
    /// Path to the risk config json5 (budget: max_loss / eloss_floor / max_pliq)
    pub risk_config: String,
}

/// Replace the full correlation matrix.
#[derive(Args, Debug)]
pub struct CorrsArgs {
    /// Path to the correlation json5 ({stamp, matrix:{index,matrix}} or {index,matrix})
    pub json: String,
}

/// Create a market book (after security + models + corrs are in place).
#[derive(Args, Debug)]
pub struct AddMarketArgs {
    /// Market symbol (e.g. MINIMAX-USD)
    pub symbol: String,
}
