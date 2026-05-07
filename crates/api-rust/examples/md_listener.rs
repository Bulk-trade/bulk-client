//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use std::process;

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use bulk_client::api::{BulkWsClient, Event, Topic};
use bulk_client::api::parts::config::WSConfig;

#[derive(Parser, Debug)]
#[command(name = "md_listener", about = "Listen to ticker and L2 book updates")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "wss://exchange-wss.bulk.trade")]
    url: String,

    /// Comma-separated symbols
    #[arg(long, default_value = "BTC-USD,ETH-USD", value_delimiter = ',')]
    symbols: Vec<String>,

    /// Number of L2 book levels (0 to skip L2 subscription)
    #[arg(long, default_value_t = 10)]
    l2_levels: u32,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into())
        )
        .init();

    let args = Args::parse();

    info!("Connecting to {} for symbols: {:?}", args.url, args.symbols);
    let client = BulkWsClient::connect(WSConfig {
        url: args.url,
        symbols: args.symbols.clone(),
        signer: None,
        ..Default::default()
    })
        .await?;

    // ── Subscribe to L2 snapshots ───────────────────────────────────────
    if args.l2_levels > 0 {
        for sym in &args.symbols {
            client
                .subscribe_l2_snapshot(sym, Some(args.l2_levels))
                .await?;
        }
    }

    // ── Register event handlers ─────────────────────────────────────────
    client
        .on(Topic::Ticker, |event| {
            if let Event::Ticker(t) = event {
                info!(
                    "[TICKER] {:<10} mark={:1.2} last={:1.2} funding={:.6} vol={:.2}",
                    t.symbol, t.mark_price, t.last_price, t.funding_rate, t.volume,
                );
            }
        })
        .await;

    client
        .on(Topic::L2Snapshot, |event| {
            if let Event::L2Snapshot(book) = event {
                let (bids, asks) = &book.levels;
                let best_bid = bids.first().map(|l| l.price).unwrap_or(0.0);
                let best_ask = asks.first().map(|l| l.price).unwrap_or(0.0);
                let spread = best_ask - best_bid;
                info!(
                    "[L2]     {:<10} bid={:1.2} ask={:1.2} spread={:.2} levels={}x{}",
                    book.symbol,
                    best_bid,
                    best_ask,
                    spread,
                    bids.len(),
                    asks.len(),
                );
            }
        })
        .await;

    client
        .on(Topic::Error, |event| {
            if let Event::Error(e) = event {
                error!("[ERROR] {e}");
            }
        })
        .await;

    // ── Wait for Ctrl-C ─────────────────────────────────────────────────
    info!("Listening... press Ctrl-C to stop.");

    tokio::signal::ctrl_c().await?;
    info!("\nShutting down...");

    client.shutdown().await;
    process::exit(0);
}