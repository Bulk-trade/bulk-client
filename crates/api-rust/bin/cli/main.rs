pub mod commands;
pub mod common;
pub mod handlers;

use std::time::Duration;
use clap::{Parser, Subcommand};
use eyre::{Context};
use bulk_api::BulkHttpClient;
use bulk_api::parts::HttpConfig;
use bulk_api::transaction::TransactionSigner;
use crate::commands::{CancelAllArgs, CancelArgs, CreateMultisigArgs, FaucetArgs, PlaceArgs};
use crate::handlers::account::handle_faucet;
use crate::handlers::cancel::{handle_cancel, handle_cancel_all};
use crate::handlers::multisig::handle_create_multisig;
use crate::handlers::orders::handle_place;
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
    /// Get a drip from faucet
    ///
    /// Example: bulk faucet
    Faucet(FaucetArgs),

    /// Place a new order.
    ///
    /// Example: bulk place Buy BTC-USD 1.3@70000 --tif GTC --iso
    Place(PlaceArgs),

    /// Cancel a single order by order-id.
    ///
    /// Example: bulk cancel 9zLLfJmX6aT5jNNsZfGqheZSH8vJHgXznKMf88HDdivK
    Cancel(CancelArgs),

    /// Cancel all open orders, optionally filtered to one instrument.
    ///
    /// Example: bulk cancel-all --instrument BTC-USD
    CancelAll(CancelAllArgs),

    /// Create a multisig account from a comma-separated list of public keys.
    ///
    /// Example: bulk create-multisig 9zLL...,C6cM... --threshold 2 --lock 120 --lifetime 300
    CreateMultisig(CreateMultisigArgs),
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
        Command::Faucet(args) => {
            handle_faucet(&mut api, args).await
        },
        Command::Place(args) => {
            handle_place(&mut api, args).await
        },
        Command::Cancel(args) => {
            handle_cancel(&mut api, args).await
        },
        Command::CancelAll(args) => {
            handle_cancel_all(&mut api, args).await
        },
        Command::CreateMultisig(args) => {
            handle_create_multisig(&mut api, args).await
        },
    }
}
