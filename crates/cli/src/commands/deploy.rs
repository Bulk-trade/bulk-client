use clap::Args;

#[derive(Args, Debug)]
pub struct ConfigSecurityArgs {
    /// Path to a json5 array of securities. Currency entries must precede dependent perps.
    pub json: String,
}

#[derive(Args, Debug)]
pub struct ConfigModelArgs {
    /// Coin registration key, for example MINIMAX or MU.
    pub coin: String,
    /// Path to the model json5.
    pub json: String,
}

#[derive(Args, Debug)]
pub struct ConfigRiskMatrixArgs {
    /// Coin registration key, for example MINIMAX or MU.
    pub coin: String,
    /// Path to the risk-matrix CSV.
    pub csv: String,
    /// Path to the risk budget json5.
    pub risk_config: String,
}

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
