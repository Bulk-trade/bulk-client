//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --bin md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbol BTC-USD
//! ```

use std::process;
use std::process::exit;
use std::sync::{Arc, Mutex, MutexGuard};
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use bulk_api::api::{BulkWsClient, Event, Topic};
use bulk_api::api::parts::config::WSConfig;
use bulk_api::msgs::OrderBookLevel;

use bulk_sdk_core::{
    trade::book::L2Book,
    markets::MktId,
    data::L2Snapshot as SDKL2Snapshot,
    data::L2Delta as SDKL2Delta,
    common::Side as SDKSide,
};
use bulk_sdk_core::securities::Security;

#[derive(Parser, Debug)]
#[command(name = "md_listener", about = "Listen to ticker and L2 book updates")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "ws://localhost:12001")]
    url: String,

    /// Comma-separated symbols
    #[arg(long, default_value = "BTC-USD")]
    symbol: String,
}


/// Conversion of L2Delta
fn to_sdk_delta (
    stamp: u64,
    instrument: MktId,
    side: SDKSide,
    delta: &OrderBookLevel
) -> SDKL2Delta {
    let mut l2: SDKL2Delta = delta.into();
    l2.instrument = instrument;
    l2.stamp = stamp;
    l2.side = side;
    l2
}

fn report_book(book: &MutexGuard<L2Book>) {
    let bbo = book.bbo();
    let crossed = bbo.ask.price <= bbo.bid.price;
    info!("[BOOK]: {}{}", bbo, if crossed { ", crossed" } else { "" });
    //if crossed { exit(1); }
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    Security::initialize();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into())
        )
        .init();

    let args = Args::parse();

    info!("Connecting to {} for symbols: {:?}", args.url, args.symbol);
    let client = BulkWsClient::connect(WSConfig {
        url: args.url,
        symbols: vec![args.symbol.clone()],
        signer: None,
        ..Default::default()
    })
        .await?;

    // ── Subscribe to L2 deltas ───────────────────────────────────────
    client
        .subscribe_l2_delta(args.symbol.as_str())
        .await?;

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

    let instrument = MktId::new(args.symbol.as_str()).unwrap();
    let book = Arc::new(Mutex::new(L2Book::new(instrument)));

    // On initial snapshot: replace the whole book.
    {
        let state = book.clone();
        client.on(Topic::L2Snapshot, move |event| {
            if let Event::L2Snapshot(snap) = event {
                let newsnap: SDKL2Snapshot  = snap.into();
                let mut book = state.lock().unwrap();
                book.update_snapshot(&newsnap);
                report_book(&book);
            }
        }).await;
    }

    // On each delta: upsert changed levels (size == 0.0 ⟹ remove the level).
    {
        let state = book.clone();
        client.on(Topic::L2Delta, move |event| {
            if let Event::L2Delta(delta) = event {
                let mut book = state.lock().unwrap();

                for level in delta.levels.0.iter() {
                    book.update_delta(&to_sdk_delta(
                        delta.timestamp,
                        instrument,
                        SDKSide::Buy,
                        level));
                }
                for level in delta.levels.1.iter() {
                    book.update_delta(&to_sdk_delta(
                        delta.timestamp,
                        instrument,
                        SDKSide::Sell,
                        level));
                }
                //info!("[L2] Updated delta: {:#?}", delta);
                report_book(&book);
            }
        }).await;
    }

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