use solana_hash::Hash;

#[derive(clap::Args, Debug)]
pub struct FaucetArgs {
    /// Amount to request
    pub amount: Option<f64>,
}