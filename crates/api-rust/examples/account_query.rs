//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use bulk_client::api::parts::HttpConfig;
use bulk_client::api::BulkHttpClient;
use clap::Parser;
use solana_pubkey::Pubkey;
use std::process;
use std::str::FromStr;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "account_query", about = "Query Account")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "https://exchange-api2.bulk.trade/api/v1")]
    url: String,

    /// Account pubkey
    #[arg(long)]
    account: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    info!("Connecting to {} for account: {:?}", args.url, args.account);
    let client = BulkHttpClient::new(&HttpConfig {
        base_url: args.url,
        signer: None,
        ..Default::default()
    });

    let addr = Pubkey::from_str(args.account.as_str())?;
    let account = client?.get_account(addr).await;

    eprintln!("account: {:?}", account);
    process::exit(0);
}
