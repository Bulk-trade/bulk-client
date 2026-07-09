pub mod commands;
pub mod common;
pub mod handlers;

use crate::commands::{
    AddMarketArgs, AgentWalletArgs, CancelAllArgs, CancelArgs, ConfigArgs, ConfigCommand,
    CorrsArgs, CreateMultisigArgs, CreateSubAccountArgs, FaucetArgs, LedgerInfoArgs,
    LiquidatorConfigArgs, ModifyArgs, MultisigProposalArgs, PlaceArgs, RangeArgs,
    RemoveSubAccountArgs, RiskConfigArgs, StopArgs, TakeProfitArgs, TrailingArgs, TransferArgs,
    UpdateLeverageArgs, UpdateMultisigPolicyArgs,
};
use crate::common::submit::SubmitOptions;
use crate::common::{resolve_api_url, CliConfig};
use crate::handlers::account::{
    handle_agent_wallet, handle_create_subaccount, handle_faucet, handle_remove_subaccount,
    handle_transfer, handle_update_leverage,
};
use crate::handlers::cancel::{handle_cancel, handle_cancel_all};
use crate::handlers::conditional::{
    handle_range, handle_stop, handle_take_profit, handle_trailing,
};
use crate::handlers::deploy::{handle_add_market, handle_corrs};
use crate::handlers::multisig::{
    handle_create_multisig, handle_multisig_approve, handle_multisig_cancel,
    handle_multisig_execute, handle_multisig_reject, handle_update_multisig_policy,
};
use crate::handlers::orders::{handle_modify, handle_place};
use crate::handlers::risk::{handle_liquidator_config, handle_risk_config};
use bulk_client::parts::HttpConfig;
use bulk_client::transaction::TransactionSigner;
use bulk_client::BulkHttpClient;
use clap::{Parser, Subcommand};
use std::time::Duration;
// ---------------------------------------------------------------------------
// Top-level CLI
// ---------------------------------------------------------------------------

/// Bulk order management CLI.
///
/// Examples:
///   bulk place Buy BTC-USD 1.3@70000 --tif GTC --iso
///   bulk cancel 9zLLfJmX6aT5jNNsZfGqheZSH8vJHgXznKMf88HDdivK
///   bulk cancel-all
///   bulk cancel-all --instrument BTC-USD
///   bulk create-multisig 9zLL...,C6cM... --threshold 2 --lock 120 --lifetime 300
///   bulk exec orders.txt
#[derive(Parser, Debug)]
#[command(name = "bulk", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Private key (base58), required unless --ledger is used.
    #[arg(long, env = "BULK_PRIVATE_KEY", hide_env_values = true, global = true)]
    private_key: Option<String>,

    /// Exchange API base URL.
    #[arg(long, global = true)]
    api_url: Option<String>,

    /// Show transaction preview before signing.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set, global = true)]
    preview: bool,

    /// Skip interactive confirmation prompt.
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

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

#[derive(Subcommand, Debug)]
enum Command {
    // ── Account ─────────────────────────────────────────────────────────────
    /// Request testnet faucet funds.
    ///
    /// Example: bulk faucet
    /// Example: bulk faucet 500
    Faucet(FaucetArgs),

    /// Show connected Ledger devices and resolved signer path/pubkey.
    #[command(name = "ledger-info")]
    LedgerInfo(LedgerInfoArgs),

    /// Persist and inspect local CLI config.
    Config(ConfigArgs),

    // ── Orders ──────────────────────────────────────────────────────────────
    /// Place a limit or market order.
    ///
    /// Example: bulk place Buy BTC-USD 1.3@70000 --tif GTC
    /// Example: bulk place Sell ETH-USD 2.0   (market)
    Place(PlaceArgs),

    /// Change the size of a resting order.
    ///
    /// Example: bulk modify BTC-USD <order-id> 0.5
    Modify(ModifyArgs),

    /// Cancel a single order by order-id.
    ///
    /// Example: bulk cancel BTC-USD <order-id>
    Cancel(CancelArgs),

    /// Cancel all open orders, optionally filtered to one instrument.
    ///
    /// Example: bulk cancel-all --instrument BTC-USD
    CancelAll(CancelAllArgs),

    // ── Conditional orders ───────────────────────────────────────────────────
    /// Place a stop order.
    ///
    /// Example: bulk stop BTC-USD 0.1 95000           (trigger below)
    /// Example: bulk stop BTC-USD 0.1 105000 --above  (trigger above)
    Stop(StopArgs),

    /// Place a take-profit order.
    ///
    /// Example: bulk tp BTC-USD 0.1 105000 --above
    #[command(name = "tp")]
    TakeProfit(TakeProfitArgs),

    /// Place a collar (stop + take-profit) order.
    ///
    /// Example: bulk range BTC-USD 0.1 90000 110000 --buy
    Range(RangeArgs),

    /// Place a trailing stop.
    ///
    /// Example: bulk trail BTC-USD 0.1 --buy --trail-bps 100 --step-bps 50
    Trail(TrailingArgs),

    // ── Settings ─────────────────────────────────────────────────────────────
    /// Update per-market maximum leverage.
    ///
    /// Example: bulk update-leverage BTC-USD=20 ETH-USD=10
    #[command(name = "update-leverage")]
    UpdateLeverage(UpdateLeverageArgs),

    /// Add or remove an agent wallet authorisation.
    ///
    /// Example: bulk agent-wallet <pubkey>
    /// Example: bulk agent-wallet <pubkey> --delete
    #[command(name = "agent-wallet")]
    AgentWallet(AgentWalletArgs),

    // ── Sub-accounts ─────────────────────────────────────────────────────────
    /// Create a named sub-account, optionally seeding it with margin.
    ///
    /// Example: bulk create-subaccount mybot --margin-symbol USDC --margin-amount 1000
    #[command(name = "create-subaccount")]
    CreateSubAccount(CreateSubAccountArgs),

    /// Remove a sub-account (must be empty).
    ///
    /// Example: bulk remove-subaccount <pubkey>
    #[command(name = "remove-subaccount")]
    RemoveSubAccount(RemoveSubAccountArgs),

    /// Transfer margin between accounts.
    ///
    /// Example: bulk transfer <from> <to> USDC 500
    Transfer(TransferArgs),

    // ── Multisig ─────────────────────────────────────────────────────────────
    /// Create a multisig account.
    ///
    /// Example: bulk create-multisig <pk1>,<pk2> --threshold 2 --lock 120
    #[command(name = "create-multisig")]
    CreateMultisig(CreateMultisigArgs),

    /// Update the policy (signers / threshold / lock) of an existing multisig.
    ///
    /// Example: bulk update-multisig <multisig> <pk1>,<pk2>,<pk3> --threshold 2
    #[command(name = "update-multisig")]
    UpdateMultisig(UpdateMultisigPolicyArgs),

    /// Approve a pending multisig proposal.
    ///
    /// Example: bulk multisig-approve <multisig> <proposal-id>
    #[command(name = "multisig-approve")]
    MultisigApprove(MultisigProposalArgs),

    /// Reject a pending multisig proposal.
    ///
    /// Example: bulk multisig-reject <multisig> <proposal-id>
    #[command(name = "multisig-reject")]
    MultisigReject(MultisigProposalArgs),

    /// Cancel a multisig proposal (proposer only).
    ///
    /// Example: bulk multisig-cancel <multisig> <proposal-id>
    #[command(name = "multisig-cancel")]
    MultisigCancel(MultisigProposalArgs),

    /// Execute an approved multisig proposal.
    ///
    /// Example: bulk multisig-execute <multisig> <proposal-id>
    #[command(name = "multisig-execute")]
    MultisigExecute(MultisigProposalArgs),

    // ── Admin ────────────────────────────────────────────────────────────
    /// Update risk configuration (JSON or file path).
    ///
    /// Example: bulk risk-config '{"max_loss":15000,...}'
    /// Example: bulk risk-config risk.json
    #[command(name = "risk-config")]
    RiskConfig(RiskConfigArgs),

    /// Update liquidator configuration (JSON or file path).
    ///
    /// Example: bulk liq-config '{"max_loss":15000,...}'
    /// Example: bulk liq-config liquidator.json
    #[command(name = "liq-config")]
    LiqConfig(LiquidatorConfigArgs),

    /// Submit a full correlation matrix.
    #[command(name = "corrs")]
    Corrs(CorrsArgs),

    /// Create a market book after configs are in place.
    #[command(name = "add-market")]
    AddMarket(AddMarketArgs),
}

impl Command {
    fn uses_deploy_timeout(&self) -> bool {
        matches!(self, Command::Corrs(_) | Command::AddMarket(_))
    }
}

fn handle_ledger_info(cli: &Cli) -> eyre::Result<()> {
    let devices = TransactionSigner::list_ledger_devices()?;
    println!("Found {} Ledger device(s).", devices.len());
    for (idx, device) in devices.iter().enumerate() {
        println!(
            "[{}] model={} serial={} host_path={} base_pubkey={}",
            idx + 1,
            device.model,
            if device.serial.is_empty() {
                "<unknown>"
            } else {
                device.serial.as_str()
            },
            if device.host_device_path.is_empty() {
                "<unknown>"
            } else {
                device.host_device_path.as_str()
            },
            device.pubkey,
        );
    }

    let resolved = TransactionSigner::resolve_ledger_with_options(
        &cli.ledger_locator,
        cli.ledger_derivation_path.as_deref(),
        cli.ledger_confirm_key,
        "bulk-cli",
    )?;
    println!();
    println!("Resolved signer:");
    println!("  locator: {}", resolved.locator);
    println!("  derivation_path: {}", resolved.derivation_path);
    println!("  resolved_path: {}", resolved.path);
    println!("  pubkey: {}", resolved.pubkey);
    Ok(())
}

fn handle_config(args: &ConfigArgs, config: &mut CliConfig) -> eyre::Result<()> {
    match &args.command {
        ConfigCommand::Show => {
            let path = crate::common::config_path()?;
            println!("config_path: {}", path.display());
            match config.api_url.as_deref() {
                Some(url) => println!("api_url: {}", url.trim_end_matches('/')),
                None => println!("api_url: <default>"),
            }
        }
        ConfigCommand::SetApiUrl { url } => {
            let value = url.trim_end_matches('/').to_string();
            config.api_url = Some(value.clone());
            config.save()?;
            println!("saved api_url={value}");
        }
        ConfigCommand::ResetApiUrl => {
            config.api_url = None;
            config.save()?;
            println!("reset api_url to default");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let mut stored_config = CliConfig::load()?;
    let api_url = resolve_api_url(cli.api_url.as_deref(), &stored_config);
    let default_timeout = if cli.command.uses_deploy_timeout() {
        Duration::from_secs(120)
    } else {
        Duration::from_secs(15)
    };
    let submit = SubmitOptions {
        preview: cli.preview,
        auto_yes: cli.yes,
    };

    if matches!(&cli.command, Command::LedgerInfo(_)) {
        return handle_ledger_info(&cli);
    }
    if let Command::Config(args) = &cli.command {
        return handle_config(args, &mut stored_config);
    }
    let signer = if cli.ledger {
        TransactionSigner::from_ledger_with_options(
            &cli.ledger_locator,
            cli.ledger_derivation_path.as_deref(),
            cli.ledger_confirm_key,
            "bulk-cli",
        )?
    } else {
        let key = cli
            .private_key
            .as_deref()
            .ok_or_else(|| eyre::eyre!("--private-key is required unless --ledger is used"))?;
        TransactionSigner::from_private_key(key)?
    };

    let config = HttpConfig {
        base_url: api_url,
        signer: Some(signer),
        default_timeout,
    };
    let mut api = BulkHttpClient::new(&config)?;

    match cli.command {
        // Account
        Command::Faucet(args) => handle_faucet(&mut api, args, &submit).await,

        // Orders
        Command::Place(args) => handle_place(&mut api, args, &submit).await,
        Command::Modify(args) => handle_modify(&mut api, args, &submit).await,
        Command::Cancel(args) => handle_cancel(&mut api, args, &submit).await,
        Command::CancelAll(args) => handle_cancel_all(&mut api, args, &submit).await,

        // Conditional orders
        Command::Stop(args) => handle_stop(&mut api, args, &submit).await,
        Command::TakeProfit(args) => handle_take_profit(&mut api, args, &submit).await,
        Command::Range(args) => handle_range(&mut api, args, &submit).await,
        Command::Trail(args) => handle_trailing(&mut api, args, &submit).await,

        // Settings
        Command::UpdateLeverage(args) => handle_update_leverage(&mut api, args, &submit).await,
        Command::AgentWallet(args) => handle_agent_wallet(&mut api, args, &submit).await,

        // Sub-accounts
        Command::CreateSubAccount(args) => handle_create_subaccount(&mut api, args, &submit).await,
        Command::RemoveSubAccount(args) => handle_remove_subaccount(&mut api, args, &submit).await,
        Command::Transfer(args) => handle_transfer(&mut api, args, &submit).await,

        // Multisig
        Command::CreateMultisig(args) => handle_create_multisig(&mut api, args, &submit).await,
        Command::UpdateMultisig(args) => {
            handle_update_multisig_policy(&mut api, args, &submit).await
        }
        Command::MultisigApprove(args) => handle_multisig_approve(&mut api, args, &submit).await,
        Command::MultisigReject(args) => handle_multisig_reject(&mut api, args, &submit).await,
        Command::MultisigCancel(args) => handle_multisig_cancel(&mut api, args, &submit).await,
        Command::MultisigExecute(args) => handle_multisig_execute(&mut api, args, &submit).await,
        Command::RiskConfig(args) => handle_risk_config(&mut api, args, &submit).await,
        Command::LiqConfig(args) => handle_liquidator_config(&mut api, args, &submit).await,
        Command::Corrs(args) => handle_corrs(&mut api, args, &submit).await,
        Command::AddMarket(args) => handle_add_market(&mut api, args, &submit).await,
        Command::LedgerInfo(_) => unreachable!("handled before API setup"),
        Command::Config(_) => unreachable!("handled before API setup"),
    }
}

//
// Unit Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ledger_info_command_without_private_key() {
        let cli = Cli::try_parse_from(["bulk", "ledger-info"]).expect("should parse ledger-info");
        assert!(matches!(cli.command, Command::LedgerInfo(_)));
        assert!(!cli.ledger);
    }

    #[test]
    fn parse_ledger_info_command_with_ledger_flags() {
        let cli = Cli::try_parse_from([
            "bulk",
            "ledger-info",
            "--ledger-locator",
            "usb://ledger",
            "--ledger-derivation-path",
            "0/0",
        ])
        .expect("should parse ledger-info with ledger options");
        assert!(matches!(cli.command, Command::LedgerInfo(_)));
        assert_eq!(cli.ledger_locator, "usb://ledger");
        assert_eq!(cli.ledger_derivation_path.as_deref(), Some("0/0"));
    }

    #[test]
    fn parse_config_set_api_url_command() {
        let cli = Cli::try_parse_from([
            "bulk",
            "config",
            "set-api-url",
            "https://exchange-api.bulk.trade/api/v1",
        ])
        .expect("should parse config set-api-url");
        match cli.command {
            Command::Config(ConfigArgs {
                command: ConfigCommand::SetApiUrl { url },
            }) => {
                assert_eq!(url, "https://exchange-api.bulk.trade/api/v1");
            }
            _ => panic!("expected config set-api-url"),
        }
    }
}
