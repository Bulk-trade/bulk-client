use std::str::FromStr;
use solana_hash::Hash;

// ---------------------------------------------------------------------------
// Update risk
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct RiskConfigArgs {
    // Json or filename containing risk config json
    pub(crate) json: String,
}


