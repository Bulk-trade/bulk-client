// ---------------------------------------------------------------------------
// Update risk
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct RiskConfigArgs {
    // Json or filename containing risk config json
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
// Update liquidator
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct LiquidatorConfigArgs {
    // Json or filename containing liquidator config json
    pub(crate) json: String,
}
