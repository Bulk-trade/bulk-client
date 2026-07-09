// ---------------------------------------------------------------------------
// CreateMultisigArgs
// ---------------------------------------------------------------------------

use solana_pubkey::Pubkey;
use std::str::FromStr;

#[derive(clap::Args, Debug)]
pub struct CreateMultisigArgs {
    /// Signer public keys (base58), space-separated.
    #[arg(value_parser = parse_pubkey)]
    pub signers: Vec<Pubkey>,

    /// Number of signers required to approve a transaction.
    #[arg(long)]
    pub threshold: u32,

    /// Lock period in seconds before the multisig can be modified.
    #[arg(long, default_value = "0")]
    pub lock: u32,

    /// Proposal lifetime in seconds (min 3600 = 1h, max 7776000 = 90d).
    #[arg(long)]
    pub lifetime: u32,
}

// ---------------------------------------------------------------------------
// UpdateMultisigPolicyArgs
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct UpdateMultisigPolicyArgs {
    /// Multisig account public key (base58).
    #[arg(value_parser = parse_pubkey)]
    pub multisig: Pubkey,

    /// New signer public keys (base58), space-separated.
    #[arg(value_parser = parse_pubkey)]
    pub signers: Vec<Pubkey>,

    /// New approval threshold.
    #[arg(long)]
    pub threshold: u32,

    /// New time-lock duration in seconds.
    #[arg(long, default_value = "0")]
    pub lock: u32,

    /// New proposal lifetime in seconds.
    #[arg(long, default_value = "0")]
    pub lifetime: u32,
}

// ---------------------------------------------------------------------------
// Proposal operation args (Approve / Reject / Cancel / Execute all share shape)
// ---------------------------------------------------------------------------

/// Args shared by approve, reject, cancel, and execute.
#[derive(clap::Args, Debug)]
pub struct MultisigProposalArgs {
    /// Multisig account public key (base58).
    #[arg(value_parser = parse_pubkey)]
    pub multisig: Pubkey,

    /// Proposal ID to act on.
    pub proposal_id: u64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_pubkey(s: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(s).map_err(|e| e.to_string())
}
