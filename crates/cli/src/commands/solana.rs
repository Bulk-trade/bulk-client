use solana_pubkey::Pubkey;
use std::str::FromStr;

fn parse_pubkey(s: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(s).map_err(|e| e.to_string())
}

/// Mainnet bulk program id.
pub const DEFAULT_PROGRAM_ID: &str = "BULK2CNYn3mbgfYXEXiBBFxmmDChznpjQ4oRfce8w6R4";

/// Deposit tokens into a vault (opcode 2): caller token account → vault.
///
/// Example: bulk deposit --mint EPjF... --amount 1000000
#[derive(clap::Args, Debug)]
pub struct DepositArgs {
    /// Token mint (base58).
    #[arg(long, value_parser = parse_pubkey)]
    pub mint: Pubkey,

    /// Amount in mint base units (e.g. 1_000_000 = 1 USDC).
    #[arg(long)]
    pub amount: u64,

    /// Source token account (default: caller's ATA for the mint).
    #[arg(long, value_parser = parse_pubkey)]
    pub user_token_account: Option<Pubkey>,

    /// Bulk program id.
    #[arg(long, default_value = DEFAULT_PROGRAM_ID, value_parser = parse_pubkey, env = "BULK_PROGRAM_ID")]
    pub program_id: Pubkey,

    /// Solana RPC URL.
    #[arg(long, env = "BULK_RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    pub rpc_url: String,

    /// Build and print the instruction without sending.
    #[arg(long)]
    pub dry_run: bool,
}

/// Signal-only withdraw intent (opcode 4): no transfer; Bulk locks margin from the plugin event.
///
/// Example: bulk withdraw-intent --mint EPjF... --amount 1000000
#[derive(clap::Args, Debug)]
pub struct WithdrawIntentArgs {
    /// Token mint (base58).
    #[arg(long, value_parser = parse_pubkey)]
    pub mint: Pubkey,

    /// Amount in mint base units (e.g. 1_000_000 = 1 USDC).
    #[arg(long)]
    pub amount: u64,

    /// Recipient token account (default: caller's ATA for the mint).
    #[arg(long, value_parser = parse_pubkey)]
    pub user_token_account: Option<Pubkey>,

    /// Bulk program id.
    #[arg(long, default_value = DEFAULT_PROGRAM_ID, value_parser = parse_pubkey, env = "BULK_PROGRAM_ID")]
    pub program_id: Pubkey,

    /// Solana RPC URL.
    #[arg(long, env = "BULK_RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    pub rpc_url: String,

    /// Build and print the instruction without sending.
    #[arg(long)]
    pub dry_run: bool,
}
