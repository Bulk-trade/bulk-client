//! ═══════════════════════════════════════════════════════════════════════════
//! Random Market Maker
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::distr::{Distribution, Uniform};
use tokio::sync::watch;
use tracing::{error, info, warn};
use bulk_api::api::BulkHttpClient;
use bulk_api::api::parts::HttpConfig;
use bulk_api::common::tif::TimeInForce;
use bulk_api::msgs::{CancelAll, LimitOrder};
use bulk_api::transaction::{Action, TransactionSigner};
use crate::parts::config::RmmConfig;
use crate::parts::price_process::OuProcess;

/// Random market maker for WS
pub struct RandomHttpMarketMaker {
    config: RmmConfig,
    client: BulkHttpClient,
    ou: OuProcess,
    running: watch::Receiver<bool>,
    stop_tx: watch::Sender<bool>,
    total_orders: u64,
    tick_count: u64,
}

#[allow(unused)]
impl RandomHttpMarketMaker {
    // ── Construction ──────────────────────────────────────────────────────

    /// Connect to the exchange and prepare the market maker.
    pub async fn initialize(config: RmmConfig, private_key: &str) -> eyre::Result<Self> {
        info!("{}", "=".repeat(70));
        info!("RANDOM MARKET MAKER — INITIALIZATION");
        info!("{}", "=".repeat(70));

        let signer = TransactionSigner::from_private_key(private_key)?;
        let pubkey = signer.public_key();
        info!("Account: {}", signer.public_key_b58());

        let symbol = config.bulk_symbol();
        info!(
            "Connecting to Bulk ({}) on {} ...",
            symbol, config.url
        );

        let http_config = HttpConfig {
            base_url: config.url.clone(),
            signer: Some(signer),
            default_timeout: Duration::from_secs(10),
        };

        let client = BulkHttpClient::new(&http_config)?;
        info!("Connected to Bulk");

        // Wait for account snapshot
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Cancel any stale orders
        client.cancel_all(vec![symbol.clone()], None, None).await?;
        info!("Cancelled any pre-existing orders");

        let ou = OuProcess::new(config.price, config.kappa, config.sigma);

        let (stop_tx, running) = watch::channel(true);

        info!("{}", "=".repeat(70));
        info!("INITIALIZATION COMPLETE");
        info!(
            "  mid={}  kappa={}  sigma={}  spread={}",
            config.price, config.kappa, config.sigma, config.spread
        );
        info!(
            "  depth={}  orders/level={}  freq={}s",
            config.depth, config.orders_per_level, config.frequency
        );
        info!("{}", "=".repeat(70));

        Ok(Self {
            config,
            client,
            ou,
            running,
            stop_tx,
            total_orders: 0,
            tick_count: 0,
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

        loop {
            if !*self.running.borrow() {
                break;
            }

            let t0 = Instant::now();

            // evaluate next time step
            if let Err(e) = self.tick().await {
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
        if let Err(e) = self
            .client
            .cancel_all(vec![self.config.bulk_symbol()], None, None)
            .await
        {
            error!("Error cancelling on shutdown: {e}");
        }
        info!(
            "Done. Ticks={}  TotalOrders={}",
            self.tick_count, self.total_orders
        );
    }

    // ── Core tick ────────────────────────────────────────────────────────

    async fn tick(&mut self) -> eyre::Result<()> {
        self.tick_count += 1;
        info!("evaluating tick: {}", self.tick_count);
        let nonce_base = make_nonce();

        // 1. Evolve mid price
        let mid = self.ou.step(self.config.frequency);

        // 2. Periodically send oracle updates
        let mut nonce = nonce_base;

        // 3. Build and send order book (skip during priming)
        let orders = self.build_book(mid);
        let n_orders = orders.len();

        // First chunk includes cancel-all; remaining chunks are orders only
        let cancel = Action::CancelAll(CancelAll {
            symbols: vec![],
            meta: Default::default()
        });

        let chunk_size = self.config.chunksize;
        let mut chunk_start = 0;
        let mut first = true;

        while chunk_start < orders.len() || first {
            let chunk_end = (chunk_start + chunk_size).min(orders.len());
            let mut actions: Vec<Action> = Vec::new();

            if first {
                actions.push(cancel.clone().into());
                first = false;
            }

            actions.extend(orders[chunk_start..chunk_end].to_vec());

            // place orders
            match self.client.place_tx(actions, None, Some(nonce)).await {
                Ok(responses) => {
                    let n_err = responses.iter().filter(|r| r.is_error()).count();
                    if n_err > 1 {
                        warn!(
                            "Tick {}: {}/{} errors",
                            self.tick_count,
                            n_err,
                            responses.len()
                        );
                    }
                }
                Err(e) => {
                    error!("Tick {} chunk send error: {e}", self.tick_count);
                }
            }

            nonce += 1;
            chunk_start = chunk_end;
        }

        self.total_orders += n_orders as u64;

        if self.tick_count % 100 == 0 {
            info!(
                "Tick {}: mid=${mid:.2}  orders={n_orders}  total={}",
                self.tick_count, self.total_orders
            );
        }

        Ok(())
    }

    // ── Book generation ──────────────────────────────────────────────────

    /// Generate a full symmetric order book around `mid`.
    ///
    /// - Spread between best bid / best ask = `config.spread`
    /// - Level spacing = spread (uniform tick)
    /// - Size grows linearly with distance from mid:
    ///       `level_size = base_size * (1 + level_index * 0.01)`
    ///   Each order at that level = `level_size / orders_per_level`
    fn build_book(&self, mid: f64) -> Vec<Action> {
        let cfg = &self.config;
        let half_spread = cfg.spread / 2.0;
        let growth = 0.01;
        let symbol = cfg.bulk_symbol();

        let mut rng = rand::rng();
        let jitter = Uniform::new(0.5_f64, 1.5).unwrap();

        let mut orders = Vec::with_capacity(cfg.depth * cfg.orders_per_level * 2);

        for level in 0..cfg.depth {
            let offset = half_spread + (level as f64) * cfg.spread;
            let bid_price = round2(mid - offset);
            let ask_price = round2(mid + offset);

            let level_size = cfg.size * (1.0 + level as f64 * growth);
            let jitter_bid: f64 = jitter.sample(&mut rng);
            let jitter_ask: f64 = jitter.sample(&mut rng);

            let order_size_bid = round6(level_size * jitter_bid / cfg.orders_per_level as f64);
            let order_size_ask = round6(level_size * jitter_ask / cfg.orders_per_level as f64);

            for i in 0..cfg.orders_per_level {
                let bid_sz = order_size_bid + (i + 1) as f64 * 1e-5;
                let bid_order = LimitOrder {
                    symbol: Arc::from(symbol.as_str()),
                    is_buy: true,
                    price: bid_price,
                    size: bid_sz,
                    tif: TimeInForce::ALO,
                    reduce_only: false,
                    meta: Default::default(),
                };
                orders.push(bid_order.into());

                let ask_sz = order_size_ask + (i + 1) as f64 * 1e-5;
                let ask_order = LimitOrder {
                    symbol: Arc::from(symbol.as_str()),
                    is_buy: false,
                    price: ask_price,
                    size: ask_sz,
                    tif: TimeInForce::ALO,
                    reduce_only: false,
                    meta: Default::default(),
                };
                orders.push(ask_order.into());
            }
        }

        orders
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn make_nonce() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}
