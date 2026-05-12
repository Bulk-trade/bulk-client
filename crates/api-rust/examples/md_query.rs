//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use std::process;
use clap::Parser;
use tracing::{info};
use tracing_subscriber::EnvFilter;
use bulk_client::api::{BulkHttpClient};
use bulk_client::api::parts::HttpConfig;

#[derive(Parser, Debug)]
#[command(name = "md_query", about = "Query MD")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "https://exchange-api2.bulk.trade/api/v1")]
    url: String,

    /// symbol to query
    #[arg(long, default_value = "BTC-USD")]
    symbol: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into())
        )
        .init();

    let args = Args::parse();

    info!("Connecting to {} for symbol: {:?}", args.url, args.symbol);
    let client = BulkHttpClient::new(&HttpConfig {
        base_url: args.url,
        signer: None,
        ..Default::default()
    }).unwrap();

    let book = client.get_orderbook(&args.symbol, None, None).await?;
    eprintln!("book: {:?}\n", book);

    let ticker = client.get_ticker(&args.symbol).await?;
    eprintln!("ticker: {:?}\n", ticker);

    let markets = client.get_exchange_info().await?;
    eprintln!("markets: {:?}\n", markets);

    process::exit(0);
}