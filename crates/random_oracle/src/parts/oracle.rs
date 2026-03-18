//! ═══════════════════════════════════════════════════════════════════════════
//! Random Market Maker
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use rand::distr::{Distribution, Uniform};
use tokio::sync::watch;
use tracing::{error, info, warn};
use bulk_api::api::BulkWsClient;
use bulk_api::api::parts::config::WSConfig;
use bulk_api::common::tif::TimeInForce;
use bulk_api::msgs::{CancelAll, LimitOrder, Price};
use bulk_api::transaction::{Action, TransactionSigner};
use crate::parts::config::OracleConfig;
use crate::parts::price_process::OuProcess;

/// Random market maker for WS
pub struct RandomOracle {
    config: OracleConfig,
    client: BulkWsClient,
    prices: Vec<OuProcess>,
    running: watch::Receiver<bool>,
    stop_tx: watch::Sender<bool>,
}

impl RandomOracle {
    // ── Construction ──────────────────────────────────────────────────────

    /// Connect to the exchange and prepare the market maker.
    pub async fn initialize(config: OracleConfig, private_key: &str) -> eyre::Result<Self> {
        info!("{}", "=".repeat(70));
        info!("RANDOM ORACLE — INITIALIZATION");
        info!("{}", "=".repeat(70));

        let signer = TransactionSigner::from_private_key(private_key)?;
        info!("Account: {}", signer.public_key_b58());

        let ws_config = WSConfig {
            url: config.url.clone(),
            symbols: vec![],
            signer: Some(signer),
            default_timeout: Duration::from_secs(config.timeout),
            track_ticker: false,
            track_account: false,
        };

        let client = BulkWsClient::connect(ws_config).await?;
        info!("Connected to Bulk");

        let prices = config.coins.iter()
            .map(|x| OuProcess::new(x.price,x.kappa,x.sigma))
            .collect();

        let (stop_tx, running) = watch::channel(true);

        Ok(Self {
            config,
            client,
            prices,
            running,
            stop_tx,
        })
    }

    /// Signal the MM to stop after the current tick.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(false);
    }

    /// Get a cloneable stop handle for use in signal handlers.
    pub fn stop_handle(&self) -> watch::Sender<bool> {
        self.stop_tx.clone()
    }

    // ── Main loop ────────────────────────────────────────────────────────

    /// Run the market-making loop until stopped.
    pub async fn run(&mut self) -> eyre::Result<()> {
        info!("Starting random MM loop …");

        let freq = Duration::from_secs_f64(self.config.frequency);
        let mut tick_count = 0;

        loop {
            if !*self.running.borrow() {
                break;
            }

            let t0 = Instant::now();

            // evaluate next time step
            tick_count += 1;
            if let Err(e) = self.tick(tick_count).await {
                error!("Tick error: {e}");
            }

            // sleep until next update
            let elapsed = t0.elapsed();
            if elapsed < freq {
                tokio::time::sleep(freq - elapsed).await;
            }
        }

        self.shutdown().await;
        Ok(())
    }

    async fn shutdown(&self) {
        info!("Shutting down …");
        self.client.shutdown().await;
    }

    // ── Core tick ────────────────────────────────────────────────────────

    async fn tick(&mut self, tickno: usize) -> eyre::Result<()> {
        info!("evaluating tick: {}", tickno);
        let now = Utc::now().timestamp_nanos_opt().unwrap() as u64;

        let mut actions = Vec::<Price>::new();
        for (i, px) in self.prices.iter_mut().enumerate() {
            let mid = px.step(1.0);
            let price = Price {
                timestamp: now,
                asset: self.config.coins[i].coin.clone(),
                price: mid,
                meta: Default::default()
            };
            actions.push(price);
        }
        self.client
            .update_oracle(actions, None, Some(now))
            .await?;

        Ok(())
    }
}

