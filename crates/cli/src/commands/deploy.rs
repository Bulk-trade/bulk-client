use clap::Args;

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
