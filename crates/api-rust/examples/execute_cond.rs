//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use std::{env, process};
use std::str::FromStr;
use std::sync::Arc;
use clap::Parser;
use solana_pubkey::Pubkey;
use tracing::{info};
use tracing_subscriber::EnvFilter;
use bulk_client::api::{BulkHttpClient};
use bulk_client::api::parts::HttpConfig;
use bulk_client::common::tif::TimeInForce;
use bulk_client::msgs::conditional::StopOrTP;
use bulk_client::msgs::LimitOrder;
use bulk_client::parts::make_nonce;
use bulk_client::transaction::{Action, ActionMeta, TransactionSigner};

#[derive(Parser, Debug)]
#[command(name = "md_query", about = "Query MD")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "http://localhost:12000/api/v1")]
    url: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into())
        )
        .init();

    let args = Args::parse();

    info!("Connecting to {} for execution", args.url);
    let key = env::var("BULK_PRIVATE_KEY")?;
    let signer = TransactionSigner::from_private_key(key.as_str())?;
    let client = BulkHttpClient::new(&HttpConfig {
        base_url: args.url,
        signer: Some(signer.clone()),
        ..Default::default()
    }).unwrap();

    let account = if false {
        Pubkey::from_str("8oqBACkDvyJjBoiWNbZPXrnjZvFjzUjMThbi9oahAVvH")?
    } else {
        signer.public_key()
    };
    let nonce = 1776682154418;

    let mut orders = vec![
        Action::TakeProfit(StopOrTP {
            symbol: Arc::from("BTC-USD"),
            is_above: false,
            size: 0.480894,
            threshold: 40000.0,
            limit: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            }
        }),
    ];

    let results = client.place_tx(orders, Some(account), Some(nonce)).await?;
    eprintln!("results: {:?}\n", results);

    process::exit(0);
}