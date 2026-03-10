//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use std::process;
use std::sync::Arc;
use clap::Parser;
use tracing::{info};
use tracing_subscriber::EnvFilter;
use bulk_api::api::{BulkHttpClient};
use bulk_api::api::parts::HttpConfig;
use bulk_api::common::tif::TimeInForce;
use bulk_api::msgs::LimitOrder;
use bulk_api::transaction::{Action, TransactionSigner};

#[derive(Parser, Debug)]
#[command(name = "md_query", about = "Query MD")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "https://exchange-api2.bulk.trade/api/v1")]
    url: String,

    /// private key
    #[arg(long)]
    key: String,
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
    let signer = TransactionSigner::from_private_key(args.key.as_str())?;
    let client = BulkHttpClient::new(&HttpConfig {
        base_url: args.url,
        signer: Some(signer),
        ..Default::default()
    }).unwrap();

    let orders = vec![
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 1000.0,
            size: 0.0001,
            tif: TimeInForce::IOC,
            reduce_only: false,
        }),
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("ETH-USD"),
            is_buy: true,
            price: 1000.0,
            size: 0.0001,
            tif: TimeInForce::IOC,
            reduce_only: false,
        }),
    ];

    let results = client.place_tx(orders, None, None).await?;
    eprintln!("results: {:?}\n", results);

    process::exit(0);
}