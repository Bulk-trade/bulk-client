pub mod commands;
pub mod common;
pub mod handlers;

use std::time::Duration;
use clap::{Parser, Subcommand};
use bulk_client::BulkHttpClient;
use bulk_client::parts::HttpConfig;
use bulk_client::transaction::TransactionSigner;
use crate::commands::{AgentWalletArgs, CancelAllArgs, CancelArgs, CreateMultisigArgs, CreateSubAccountArgs, FaucetArgs, ModifyArgs, MultisigProposalArgs, PlaceArgs, RangeArgs, RemoveSubAccountArgs, StopArgs, TakeProfitArgs, TrailingArgs, TransferArgs, UpdateLeverageArgs, UpdateMultisigPolicyArgs};
use crate::commands::risk::RiskConfigArgs;
use crate::handlers::account::{handle_agent_wallet, handle_create_subaccount, handle_faucet, handle_remove_subaccount, handle_transfer, handle_update_leverage};
use crate::handlers::cancel::{handle_cancel, handle_cancel_all};
use crate::handlers::conditional::{handle_range, handle_stop, handle_take_profit, handle_trailing};
use crate::handlers::multisig::{handle_create_multisig, handle_multisig_approve, handle_multisig_cancel, handle_multisig_execute, handle_multisig_reject, handle_update_multisig_policy};
use crate::handlers::orders::{handle_modify, handle_place};
use crate::handlers::risk::handle_risk_config;
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

    /// Private key (base58). Defaults to BULK_PRIVATE_KEY env var.
    #[arg(long, env = "BULK_PRIVATE_KEY", hide_env_values = true)]
    private_key: String,

    /// Exchange API base URL. Defaults to BULK_API_URL env var, then the built-in default.
    #[arg(long, env = "BULK_API_URL", default_value = "http://localhost:12000/api/v1")]
    api_url: String,
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


    /// Update risk configuration
    ///
    /// Example: bulk risk-config json|json-file
    #[command(name = "risk-config")]
    RiskConfig(RiskConfigArgs),
}



#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();
    let key = cli.private_key;
    let api_url = cli.api_url.trim_end_matches('/').to_string();

    let config = HttpConfig {
        base_url: api_url,
        signer: Some(TransactionSigner::from_private_key(&key)?),
        default_timeout: Duration::from_secs(15),
    };
    let mut api = BulkHttpClient::new(&config)?;

    match cli.command {
        // Account
        Command::Faucet(args)           => handle_faucet(&mut api, args).await,

        // Orders
        Command::Place(args)            => handle_place(&mut api, args).await,
        Command::Modify(args)           => handle_modify(&mut api, args).await,
        Command::Cancel(args)           => handle_cancel(&mut api, args).await,
        Command::CancelAll(args)        => handle_cancel_all(&mut api, args).await,

        // Conditional orders
        Command::Stop(args)             => handle_stop(&mut api, args).await,
        Command::TakeProfit(args)       => handle_take_profit(&mut api, args).await,
        Command::Range(args)            => handle_range(&mut api, args).await,
        Command::Trail(args)            => handle_trailing(&mut api, args).await,

        // Settings
        Command::UpdateLeverage(args)   => handle_update_leverage(&mut api, args).await,
        Command::AgentWallet(args)      => handle_agent_wallet(&mut api, args).await,

        // Sub-accounts
        Command::CreateSubAccount(args) => handle_create_subaccount(&mut api, args).await,
        Command::RemoveSubAccount(args) => handle_remove_subaccount(&mut api, args).await,
        Command::Transfer(args)         => handle_transfer(&mut api, args).await,

        // Multisig
        Command::CreateMultisig(args)   => handle_create_multisig(&mut api, args).await,
        Command::UpdateMultisig(args)   => handle_update_multisig_policy(&mut api, args).await,
        Command::MultisigApprove(args)  => handle_multisig_approve(&mut api, args).await,
        Command::MultisigReject(args)   => handle_multisig_reject(&mut api, args).await,
        Command::MultisigCancel(args)   => handle_multisig_cancel(&mut api, args).await,
        Command::MultisigExecute(args)  => handle_multisig_execute(&mut api, args).await,

        Command::RiskConfig(args)       => handle_risk_config(&mut api, args).await,
    }
}

