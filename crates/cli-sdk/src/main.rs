use bulk_cli_sdk::{config_risk_action, config_security_action};
use bulk_client::msgs::MultisigPropose;
use bulk_client::parts::{make_nonce, HttpConfig};
use bulk_client::transaction::{
    Action, ActionMeta, ClearSignMessage, SignatureDomain, TransactionSigner,
};
use bulk_client::BulkHttpClient;
use clap::{Args, Parser, Subcommand};
use solana_pubkey::Pubkey;
use std::io::{self, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

const ADMIN_MULTISIG: &str = "ADM1N11111111111111111111111111111111111113D";
const DEFAULT_API_URL: &str = "http://localhost:12000/api/v1";

// ───── CLI Definition ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(name = "bulk-sdk", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Private key encoded as base58.
    #[arg(long, env = "BULK_PRIVATE_KEY", hide_env_values = true, global = true)]
    private_key: Option<String>,

    /// Exchange API base URL.
    #[arg(long, env = "BULK_API_URL", default_value = DEFAULT_API_URL, global = true)]
    api_url: String,

    /// Signature network domain.
    #[arg(long, env = "BULK_SIGNATURE_DOMAIN", global = true)]
    signature_domain: Option<SignatureDomain>,

    /// Show the canonical transaction before signing.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set, global = true)]
    preview: bool,

    /// Submit without interactive confirmation.
    #[arg(long, global = true)]
    yes: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Replace one coin's complete risk surface from a CSV file.
    #[command(name = "config-risk-matrix")]
    ConfigRiskMatrix(ConfigRiskMatrixArgs),

    /// Replace one complete security or currency definition.
    #[command(name = "config-security")]
    ConfigSecurity(ConfigSecurityArgs),
}

#[derive(Args, Debug)]
struct ConfigRiskMatrixArgs {
    /// Registered SDK security name, for example BTC.
    coin: String,

    /// Path to the risk-surface CSV file.
    csv: String,
}

#[derive(Args, Debug)]
struct ConfigSecurityArgs {
    /// Inline JSON/JSON5 or a path containing one security definition.
    json: String,
}

/// Wraps an SDK-backed action in an administrative proposal and submits it.
///
/// * Wraps the action for the protocol administrative multisig.
/// * Optionally renders and confirms the canonical signing message.
/// * Submits the signed transaction and prints each returned status.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `action` - SDK-backed public action to propose.
/// * `preview` - Whether to display the canonical signing message.
/// * `auto_yes` - Whether to skip interactive confirmation.
///
/// # Returns
/// An error when proposal construction, confirmation, signing, or submission fails.
async fn submit_action(
    api: &mut BulkHttpClient,
    action: Action,
    preview: bool,
    auto_yes: bool,
) -> eyre::Result<()> {
    let admin_multisig = Pubkey::from_str(ADMIN_MULTISIG)?;
    let actions = vec![Action::MultisigPropose(MultisigPropose {
        multisig: admin_multisig,
        actions: vec![action],
        proposal_lifetime_secs: None,
        meta: ActionMeta::default(),
    })];
    let nonce = make_nonce();

    if preview {
        preview_and_confirm(api, nonce, &actions, auto_yes)?;
    }

    for response in api.place_tx(actions, None, Some(nonce)).await? {
        println!("{}", response.status);
        if let Some(message) = response.message {
            println!("  {message}");
        }
    }
    Ok(())
}

/// Displays the canonical signing message and obtains submission confirmation.
///
/// # Arguments
/// * `api` - Configured client providing the signer and signature domain.
/// * `nonce` - Transaction nonce included in the signing message.
/// * `actions` - Complete action list that will be submitted.
/// * `auto_yes` - Whether to accept the preview without reading standard input.
///
/// # Returns
/// An error when configuration is incomplete, rendering fails, or the user rejects submission.
fn preview_and_confirm(
    api: &BulkHttpClient,
    nonce: u64,
    actions: &[Action],
    auto_yes: bool,
) -> eyre::Result<()> {
    let config = api.config();
    let signer = config
        .signer
        .as_ref()
        .ok_or_else(|| eyre::eyre!("signer required"))?;
    let domain = config
        .signature_domain
        .ok_or_else(|| eyre::eyre!("signature domain required"))?;
    eprintln!("--- transaction preview ---");
    eprint!(
        "{}",
        ClearSignMessage::canonical_message(domain, signer.public_key(), nonce, actions)?
    );

    if !auto_yes {
        eprint!("Submit? [y/N]: ");
        io::stderr().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Err(eyre::eyre!("transaction rejected by user"));
        }
    }
    Ok(())
}

// ───── Input Helpers ───────────────────────────────────────────────────────────────────────

/// Resolves an argument as file contents when the path exists or as inline text otherwise.
///
/// # Arguments
/// * `input` - Candidate filesystem path or inline JSON/JSON5 value.
///
/// # Returns
/// The resolved text or an error when an existing file cannot be read.
fn read_inline_or_file(input: &str) -> eyre::Result<String> {
    if Path::new(input).exists() {
        return std::fs::read_to_string(input)
            .map_err(|error| eyre::eyre!("failed to read '{input}': {error}"));
    }
    Ok(input.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that Clap recognizes the SDK-specific command surface and global options.
    #[test]
    fn parses_sdk_specific_commands() {
        assert!(matches!(
            Cli::try_parse_from([
                "bulk-sdk",
                "config-risk-matrix",
                "BTC",
                "btc-risk.csv",
                "--private-key",
                "secret",
                "--signature-domain",
                "devnet"
            ])
            .unwrap()
            .command,
            Command::ConfigRiskMatrix(_)
        ));
    }
}

// ───── Entrypoint ──────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let action = match cli.command {
        Command::ConfigRiskMatrix(args) => config_risk_action(&args.coin, &args.csv)?,
        Command::ConfigSecurity(args) => {
            let json = read_inline_or_file(&args.json)?;
            config_security_action(&json)?
        }
    };

    let private_key = cli
        .private_key
        .as_deref()
        .ok_or_else(|| eyre::eyre!("--private-key is required"))?;
    let signature_domain = cli
        .signature_domain
        .ok_or_else(|| eyre::eyre!("--signature-domain is required"))?;
    let signer = TransactionSigner::from_private_key(private_key)?;
    let config = HttpConfig {
        base_url: cli.api_url.trim_end_matches('/').to_owned(),
        signer: Some(signer),
        signature_domain: Some(signature_domain),
        default_timeout: Duration::from_secs(120),
    };
    let mut api = BulkHttpClient::new(&config)?;
    submit_action(&mut api, action, cli.preview, cli.yes).await
}
