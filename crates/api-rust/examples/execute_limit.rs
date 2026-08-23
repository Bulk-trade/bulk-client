//! Example: Listen to ticker and L2 order book updates.
//!
//! ```bash
//! cargo run --example md_listener -- \
//!     --url wss://exchange-wss.bulk.trade \
//!     --symbols BTC-USD,ETH-USD
//! ```

use bulk_client::api::parts::HttpConfig;
use bulk_client::api::BulkHttpClient;
use bulk_client::common::tif::TimeInForce;
use bulk_client::msgs::LimitOrder;
use bulk_client::api::parts::make_nonce;

use bulk_client::transaction::{Action, ActionMeta, SignatureDomain, TransactionSigner};
use clap::Parser;
use std::sync::Arc;
use std::{env, process};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "md_query", about = "Query MD")]
struct Args {
    /// WebSocket URL
    #[arg(long, default_value = "http://localhost:12000/api/v1")]
    url: String,

    /// Network domain bound into the transaction signature.
    #[arg(long, env = "BULK_SIGNATURE_DOMAIN")]
    signature_domain: SignatureDomain,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let args = Args::parse();

    info!("Connecting to {} for execution", args.url);
    let key = env::var("BULK_PRIVATE_KEY")?;
    let signer = TransactionSigner::from_private_key(key.as_str())?;
    let client = BulkHttpClient::new(&HttpConfig {
        base_url: args.url,
        signer: Some(signer.clone()),
        signature_domain: Some(args.signature_domain),
        ..Default::default()
    })
    .unwrap();

    let account = signer.public_key();
    let nonce = make_nonce();

    let mut orders = vec![
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 1000.0,
            size: 0.0001,
            tif: TimeInForce::IOC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        }),
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("ETH-USD"),
            is_buy: true,
            price: 1000.0,
            size: 0.0001,
            tif: TimeInForce::IOC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 1,
                hash: None,
            },
        }),
    ];

    let oids = orders
        .iter_mut()
        .map(|o| o.hash().to_string())
        .collect::<Vec<_>>();

    eprintln!("order IDs: {:?}", oids);

    let results = client.place_tx(orders, Some(account), Some(nonce)).await?;
    eprintln!("results: {:?}\n", results);

    process::exit(0);
}
