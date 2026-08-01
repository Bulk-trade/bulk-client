// ---------------------------------------------------------------------------
// Faucet
// ---------------------------------------------------------------------------

use bulk_client::msgs::subaccounts::TransferKind;
use std::str::FromStr;
use solana_pubkey::Pubkey;

#[derive(clap::Args, Debug)]
pub struct FaucetArgs {
    /// Amount to request
    pub amount: Option<f64>,
}

#[derive(clap::Args, Debug)]
pub struct LedgerInfoArgs {}

// ---------------------------------------------------------------------------
// Leverage
// ---------------------------------------------------------------------------

/// Update per-market maximum leverage.
///
/// Accepts one or more `SYMBOL=LEVERAGE` pairs.
///
/// Example: bulk update-leverage BTC-USD=20 ETH-USD=10
#[derive(clap::Args, Debug)]
pub struct UpdateLeverageArgs {
    /// One or more `SYMBOL=LEVERAGE` pairs, e.g. `BTC-USD=20`.
    #[arg(value_parser = parse_leverage_pair, required = true)]
    pub settings: Vec<(String, f64)>,
}

fn parse_leverage_pair(s: &str) -> Result<(String, f64), String> {
    let (sym, lev) = s
        .split_once('=')
        .ok_or_else(|| format!("expected SYMBOL=LEVERAGE, got `{s}`"))?;
    let lev: f64 = lev
        .parse()
        .map_err(|_| format!("invalid leverage `{lev}` in `{s}`"))?;
    Ok((sym.to_string(), lev))
}

// ---------------------------------------------------------------------------
// Agent wallet
// ---------------------------------------------------------------------------

/// Add or remove an agent wallet authorisation.
///
/// Example: bulk agent-wallet 9zLL... --add
/// Example: bulk agent-wallet 9zLL... --delete
#[derive(clap::Args, Debug)]
pub struct AgentWalletArgs {
    /// Agent public key (base58).
    #[arg(value_parser = parse_pubkey)]
    pub agent: Pubkey,

    /// Remove the agent instead of adding it.
    #[arg(long)]
    pub delete: bool,
}

fn parse_pubkey(s: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(s).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// CreateSubAccount
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct CreateSubAccountArgs {
    /// Human-readable name for the sub-account.
    pub name: String,

    /// Asset symbol to seed the sub-account with (e.g. USDC).
    #[arg(long)]
    pub margin_symbol: Option<String>,

    /// Amount of the asset to transfer on creation.
    #[arg(long)]
    pub margin_amount: Option<f64>,
}

// ---------------------------------------------------------------------------
// RemoveSubAccount
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct RemoveSubAccountArgs {
    /// Public key of the sub-account to remove.
    #[arg(value_parser = parse_pubkey)]
    pub pubkey: Pubkey,
}

// ---------------------------------------------------------------------------
// Transfer
// ---------------------------------------------------------------------------

#[derive(clap::Args, Debug)]
pub struct TransferArgs {
    /// Source account public key (base58).
    #[arg(value_parser = parse_pubkey)]
    pub from: Pubkey,

    /// Destination account public key (base58).
    #[arg(value_parser = parse_pubkey)]
    pub to: Pubkey,

    /// Asset symbol to transfer (e.g. USDC).
    pub symbol: String,

    /// Amount to transfer.
    pub amount: f64,

    /// `internal` (default, between your own accounts) or `external`.
    #[arg(long, default_value = "internal", value_parser = parse_transfer_kind)]
    pub kind: TransferKind,
}

fn parse_transfer_kind(s: &str) -> Result<TransferKind, String> {
    match s {
        "internal" => Ok(TransferKind::Internal),
        "external" => Ok(TransferKind::External),
        other => Err(format!(
            "unknown transfer kind `{other}`; expected `internal` or `external`"
        )),
    }
}
