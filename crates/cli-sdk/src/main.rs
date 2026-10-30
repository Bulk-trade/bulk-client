use bulk_cli_sdk::{
    config_risk_action, config_security_actions, create_market_actions,
    create_named_market_actions, market_liquidator_action,
};
use bulk_client::msgs::{MarketAction, MarketAdmin, MultisigPropose, Response};
use bulk_client::parts::{make_nonce, HttpConfig};
use bulk_client::transaction::{
    Action, ActionMeta, ClearSignMessage, SignatureDomain, TransactionSigner,
};
use bulk_client::BulkHttpClient;
use clap::{Args, Parser, Subcommand};
use solana_pubkey::Pubkey;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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

    /// Private key encoded as base58, required unless --ledger is used.
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

    /// Use Ledger (Solana app) signer mode.
    #[arg(long, global = true)]
    ledger: bool,

    /// Ledger locator, typically usb://ledger.
    #[arg(long, default_value = "usb://ledger", global = true)]
    ledger_locator: String,

    /// Ledger derivation path (key path like 0/0 or absolute path like m/44'/501'/0'/0').
    #[arg(long, global = true)]
    ledger_derivation_path: Option<String>,

    /// Confirm the selected Ledger pubkey on-device during initialization.
    #[arg(long, global = true)]
    ledger_confirm_key: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Submit all market configuration actions in one admin proposal.
    CreateBip1(CreateBip1Args),

    /// Submit a market proposal using symbol-specific models and risk configuration.
    CreateMarket(CreateMarketArgs),

    /// Replace one coin's complete risk surface from a CSV file.
    #[command(name = "config-risk-matrix")]
    ConfigRiskMatrix(ConfigRiskMatrixArgs),

    /// Replace security or currency definitions in one admin multisig proposal.
    #[command(name = "config-security")]
    ConfigSecurity(ConfigSecurityArgs),
}

#[derive(Args, Debug)]
struct CreateBip1Args {
    #[command(flatten)]
    market: CreateMarketArgs,
    /// BIP1 reference tier.
    #[arg(value_parser = ["low", "medium", "high"])]
    tier: String,
}

#[derive(Args, Debug)]
struct CreateMarketArgs {
    /// Underlier currency name; the traded instrument and model filenames use SYMBOL.
    symbol: String,
    /// Root directory containing securities, models, and risk configurations.
    #[arg(long)]
    config_dir: PathBuf,
    /// Executor JSON5 file supplying funding defaults and pair overrides.
    #[arg(long)]
    executor_config: PathBuf,
    /// Omit the currency action when the underlier already exists on the exchange.
    #[arg(long)]
    skip_currency: bool,
    /// Fetch current liquidator config, validate, and print actions without a signer.
    #[arg(long)]
    dry_run: bool,
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
    /// Inline JSON/JSON5 or a file containing one security definition or an array.
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
/// * `proposed` - SDK-backed public actions to propose.
/// * `preview` - Whether to display the canonical signing message.
/// * `auto_yes` - Whether to skip interactive confirmation.
///
/// # Returns
/// An error when proposal construction, confirmation, signing, or submission fails.
async fn submit_action(
    api: &mut BulkHttpClient,
    proposed: Vec<Action>,
    preview: bool,
    auto_yes: bool,
) -> eyre::Result<()> {
    let admin_multisig = Pubkey::from_str(ADMIN_MULTISIG)?;
    let actions = vec![Action::MultisigPropose(MultisigPropose {
        multisig: admin_multisig,
        actions: proposed,
        proposal_lifetime_secs: None,
        meta: ActionMeta::default(),
    })];
    let nonce = make_nonce();

    if preview {
        preview_and_confirm(api, nonce, &actions, auto_yes)?;
    }

    let action_debug = proposed_action_debug(&actions);
    let results = api.place_tx(actions, None, Some(nonce)).await?;
    eprint!("{}", format_results(&results, &action_debug));
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

// ───── Response Formatting ─────────────────────────────────────────────────────────────────

/// Formats submission responses using the same output layout as the main CLI.
///
/// # Arguments
/// * `results` - Ordered API responses returned for the submitted transaction.
/// * `action_debug` - Debug descriptions of actions nested inside a proposal.
fn format_results(results: &[Response], action_debug: &[String]) -> String {
    if let Some(created) = results
        .iter()
        .find(|response| response.status == "proposalCreated")
    {
        return format_proposal_created(created, action_debug);
    }

    let approval = results
        .iter()
        .find(|response| response.status == "proposalApproved");
    let proposal_outcome = results.iter().rev().find(|response| {
        matches!(
            response.status.as_str(),
            "proposalFailed"
                | "proposalExecuted"
                | "proposalReadyForExecution"
                | "proposalRejected"
        )
    });

    if approval.is_some() || proposal_outcome.is_some() {
        return format_approval_result(approval, proposal_outcome);
    }

    let mut output = String::from("\nStatus\n");
    output.push_str("────────────────────────────────────────\n");
    if results.is_empty() {
        output.push_str("  No response statuses returned\n");
        return output;
    }
    for response in results {
        let _ = writeln!(output, "  {}", humanize_status(&response.status));
        if let Some(message) = &response.message {
            let _ = writeln!(output, "    Message: {message}");
        }
    }
    output
}

/// Formats a newly created proposal and its nested actions.
///
/// # Arguments
/// * `created` - The proposal-created API response.
/// * `action_debug` - Debug descriptions of actions nested inside the proposal.
fn format_proposal_created(created: &Response, action_debug: &[String]) -> String {
    let mut output = String::from("\nProposal Created\n");
    output.push_str("────────────────────────────────────────\n");
    write_json_field(&mut output, "Proposal", &created.raw, "proposalId");
    write_json_field(&mut output, "Required signers", &created.raw, "threshold");
    output.push_str("\nActions\n");
    output.push_str("────────────────────────────────────────\n");
    if action_debug.is_empty() {
        output.push_str("  No action details available\n");
    } else {
        for (index, action) in action_debug.iter().enumerate() {
            let _ = writeln!(output, "  [{index}] {action}");
        }
    }
    output
}

/// Extracts debug descriptions for actions nested inside multisig proposals.
///
/// # Arguments
/// * `actions` - Submitted top-level actions.
fn proposed_action_debug(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .flat_map(|action| match action {
            Action::MultisigPropose(proposal) => proposal
                .actions
                .iter()
                .map(|nested| format!("{nested:?}"))
                .collect(),
            ordinary => vec![format!("{ordinary:?}")],
        })
        .collect()
}

/// Formats proposal approval progress and its final outcome.
///
/// # Arguments
/// * `approval` - Optional proposal-approval response.
/// * `outcome` - Optional final proposal-outcome response.
fn format_approval_result(approval: Option<&Response>, outcome: Option<&Response>) -> String {
    let details = approval.or(outcome).expect("approval result exists");
    let mut output = String::from("\nApprovals\n");
    output.push_str("────────────────────────────────────────\n");
    write_json_field(&mut output, "Proposal", &details.raw, "proposalId");
    write_json_field(&mut output, "Multisig", &details.raw, "multisig");
    let approvals = details
        .raw
        .get("approvals")
        .and_then(|value| value.as_u64());
    let threshold = details
        .raw
        .get("threshold")
        .and_then(|value| value.as_u64());
    if let (Some(approvals), Some(threshold)) = (approvals, threshold) {
        let _ = writeln!(output, "  Progress: {approvals} / {threshold} approvals");
    }
    write_json_field(&mut output, "Rejections", &details.raw, "rejections");
    let rejected = outcome
        .or(approval)
        .is_some_and(|response| response.status == "proposalRejected");
    let signer_label = if rejected { "Signer" } else { "Approved by" };
    write_json_field(&mut output, signer_label, &details.raw, "signer");

    output.push_str("\nOutcome\n");
    output.push_str("────────────────────────────────────────\n");
    let final_response = outcome.or(approval).expect("approval result exists");
    let _ = writeln!(
        output,
        "  Status: {}",
        humanize_status(&final_response.status)
    );
    if let Some(message) = &final_response.message {
        let _ = writeln!(output, "  Error: {message}");
    } else if final_response.status == "proposalReadyForExecution" {
        write_json_field(
            &mut output,
            "Execute after",
            &final_response.raw,
            "executeAfter",
        );
    }
    output
}

/// Writes one non-null JSON field as a labeled output line.
///
/// # Arguments
/// * `output` - Destination output buffer.
/// * `label` - Human-readable field label.
/// * `body` - Structured API response body.
/// * `field` - JSON field name to read.
fn write_json_field(output: &mut String, label: &str, body: &serde_json::Value, field: &str) {
    let Some(value) = body.get(field) else {
        return;
    };
    if value.is_null() {
        return;
    }
    let display = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    let _ = writeln!(output, "  {label}: {display}");
}

/// Converts a camel-case response status into a human-readable phrase.
///
/// # Arguments
/// * `status` - Machine-readable API status.
fn humanize_status(status: &str) -> String {
    let mut output = String::with_capacity(status.len() + 4);
    for (index, character) in status.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            output.push(' ');
        }
        if index == 0 {
            output.extend(character.to_uppercase());
        } else {
            output.push(character.to_ascii_lowercase());
        }
    }
    output
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
    use bulk_client::msgs::OpaqueAction;

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

    #[test]
    fn parses_create_bip1_without_signer_for_dry_run() {
        let cli = Cli::try_parse_from([
            "bulk-sdk",
            "create-bip1",
            "PAPER",
            "high",
            "--config-dir",
            "config",
            "--executor-config",
            "config/executor-testnet.json5",
            "--dry-run",
            "--skip-currency",
        ])
        .unwrap();
        let Command::CreateBip1(args) = cli.command else {
            panic!("expected CreateBip1");
        };
        assert!(args.market.dry_run && args.market.skip_currency);
        assert_eq!(args.market.symbol, "PAPER");
        assert_eq!(args.tier, "high");
        assert!(Cli::try_parse_from([
            "bulk-sdk",
            "create-bip1",
            "PAPER",
            "invalid",
            "--config-dir",
            "config",
            "--executor-config",
            "executor.json5"
        ])
        .is_err());
    }

    /// Verifies that the SDK CLI accepts the Ledger options used by the admin wrappers.
    #[test]
    fn parses_ledger_signer_options() {
        let cli = Cli::try_parse_from([
            "bulk-sdk",
            "config-security",
            "/tmp/paxg-usd.json5",
            "--ledger",
            "--ledger-derivation-path",
            "0/0",
            "--signature-domain",
            "devnet",
        ])
        .unwrap();

        assert!(cli.ledger);
        assert_eq!(cli.ledger_derivation_path.as_deref(), Some("0/0"));
        assert!(matches!(cli.command, Command::ConfigSecurity(_)));
    }

    /// Verifies that created proposals include their ID, threshold, and nested SDK action.
    #[test]
    fn formats_created_proposal_like_main_cli() {
        let nested = Action::ConfigSecurity(OpaqueAction {
            payload: vec![1, 2, 3],
            meta: ActionMeta::default(),
        });
        let actions = vec![Action::MultisigPropose(MultisigPropose {
            multisig: Pubkey::from_str(ADMIN_MULTISIG).unwrap(),
            actions: vec![nested],
            proposal_lifetime_secs: None,
            meta: ActionMeta::default(),
        })];
        let response = Response {
            order_id: None,
            status: "proposalCreated".to_owned(),
            message: None,
            raw: serde_json::json!({
                "proposalId": 42,
                "threshold": 2
            }),
        };

        let output = format_results(&[response], &proposed_action_debug(&actions));

        assert!(output.contains("Proposal Created"));
        assert!(output.contains("Proposal: 42"));
        assert!(output.contains("Required signers: 2"));
        assert!(output.contains("[0] ConfigSecurity"));
        assert!(!output.contains("MultisigPropose"));
    }
}

/// Builds market actions, applies the local liquidator policy, and suspends the new pair.
async fn prepare_market_actions(
    api_url: &str,
    args: &CreateMarketArgs,
    tier: Option<&str>,
) -> eyre::Result<Vec<Action>> {
    let mut actions = match tier {
        Some(tier) => create_market_actions(
            &args.config_dir,
            &args.executor_config,
            &args.symbol,
            tier,
            args.skip_currency,
        )?,
        None => create_named_market_actions(
            &args.config_dir,
            &args.executor_config,
            &args.symbol,
            args.skip_currency,
        )?,
    };
    let api = BulkHttpClient::new(&HttpConfig {
        base_url: api_url.trim_end_matches('/').to_owned(),
        signer: None,
        signature_domain: None,
        default_timeout: Duration::from_secs(120),
    })?;
    let config = api.get_liquidator_config().await?;
    actions.push(market_liquidator_action(
        config,
        &std::fs::read_to_string(&args.executor_config)?,
        &format!("{}-USD", args.symbol),
    )?);
    actions.push(Action::MarketAdmin(MarketAdmin {
        symbol: format!("{}-USD", args.symbol).into(),
        action: MarketAction::Suspend,
        price: None,
        meta: Default::default(),
    }));
    Ok(actions)
}

// ───── Entrypoint ──────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let (actions, dry_run) = match cli.command {
        Command::CreateBip1(args) => (
            prepare_market_actions(&cli.api_url, &args.market, Some(&args.tier)).await?,
            args.market.dry_run,
        ),
        Command::CreateMarket(args) => (
            prepare_market_actions(&cli.api_url, &args, None).await?,
            args.dry_run,
        ),
        Command::ConfigRiskMatrix(args) => {
            (vec![config_risk_action(&args.coin, &args.csv)?], false)
        }
        Command::ConfigSecurity(args) => {
            let json = read_inline_or_file(&args.json)?;
            (config_security_actions(&json)?, false)
        }
    };
    if dry_run {
        println!("{}", serde_json::to_string_pretty(&actions)?);
        return Ok(());
    }

    let signature_domain = cli
        .signature_domain
        .ok_or_else(|| eyre::eyre!("--signature-domain is required"))?;
    let signer = if cli.ledger {
        TransactionSigner::from_ledger_with_options(
            &cli.ledger_locator,
            cli.ledger_derivation_path.as_deref(),
            cli.ledger_confirm_key,
            "bulk-sdk",
        )?
    } else {
        let private_key = cli
            .private_key
            .as_deref()
            .ok_or_else(|| eyre::eyre!("--private-key is required unless --ledger is used"))?;
        TransactionSigner::from_private_key(private_key)?
    };
    let config = HttpConfig {
        base_url: cli.api_url.trim_end_matches('/').to_owned(),
        signer: Some(signer),
        signature_domain: Some(signature_domain),
        default_timeout: Duration::from_secs(120),
    };
    let mut api = BulkHttpClient::new(&config)?;
    submit_action(&mut api, actions, cli.preview, cli.yes).await
}
