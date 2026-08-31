// ---------------------------------------------------------------------------
// Update risk
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct RiskConfigArgs {
    // Json or filename containing risk config json
    pub(crate) json: String,
}

// ---------------------------------------------------------------------------
// Update funding
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct FundingConfigArgs {
    // JSON or filename containing an instrument funding config.
    pub(crate) json: String,
}

// ---------------------------------------------------------------------------
// Update account policy
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct AccountPolicyArgs {
    // JSON or filename containing an account policy update.
    pub(crate) json: String,
}

// ---------------------------------------------------------------------------
// Update user administration
// ---------------------------------------------------------------------------

/// Updates one account's open-order override and optionally the global fallback.
#[derive(clap::Args, Debug)]
#[command(group(
    clap::ArgGroup::new("account_limit")
        .required(true)
        .multiple(false)
        .args(["maxorders", "use_global"])
))]
pub struct UserAdminArgs {
    /// Logical account whose open-order override is updated.
    pub(crate) pubkey: solana_pubkey::Pubkey,

    /// Set an account-specific maximum number of resting open orders.
    #[arg(long)]
    pub(crate) maxorders: Option<usize>,

    /// Clear the account override and use the global fallback.
    #[arg(long)]
    pub(crate) use_global: bool,

    /// Also replace the executor-wide fallback limit.
    #[arg(long)]
    pub(crate) global_maxorders: Option<usize>,
}

// ---------------------------------------------------------------------------
// Update liquidator
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct LiquidatorConfigArgs {
    // Json or filename containing liquidator config json
    pub(crate) json: String,
}
