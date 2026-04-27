// ---------------------------------------------------------------------------
// CreateMultisigArgs
// ---------------------------------------------------------------------------

use std::str::FromStr;
use solana_pubkey::Pubkey;

#[derive(clap::Args, Debug)]
pub struct CreateMultisigArgs {
    /// Comma-separated list of signer public keys (base58).
    #[arg(value_parser = parse_pubkey_list)]
    pub signers: Vec<Pubkey>,

    /// Number of signers required to approve a transaction.
    #[arg(long)]
    pub threshold: u8,

    /// Lock period in seconds before the multisig can be modified.
    #[arg(long, default_value = "0")]
    pub lock: u64,

    /// Lifetime of the multisig account in seconds (0 = permanent).
    #[arg(long, default_value = "0")]
    pub lifetime: u64,
}

fn parse_pubkey_list(s: &str) -> Result<Vec<Pubkey>, String> {
    s.split(',')
        .map(|pk| Pubkey::from_str(pk.trim()).map_err(|e| e.to_string()))
        .collect()
}