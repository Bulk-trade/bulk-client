//! Bulk Labs WebSocket Trading Client — Actor + Watch architecture.
//!
//! All mutable state lives inside a single [`Actor`] task. The public
//! [`BulkWsClient`] handle is a cheap, cloneable struct that communicates
//! with the actor via:
//!
//! - **`tokio::sync::watch`** for hot-path reads (tickers, prices, margin)
//!   — zero-cost `.borrow()`, no async round-trip.
//! - **`mpsc` command channel** for writes (subscribe, place orders, etc.).
//! - **`oneshot`** for request/response flows (order placement).
//!
//! ```text
//!  ┌──────────────┐         mpsc::channel           ┌───────────────┐
//!  │ BulkWsClient │ ───── Command ────────────────▶ │     Actor     │
//!  │   (handle)   │ ◀──── watch::Receiver ────────  │  (owns state) │
//!  └──────────────┘                                 └───────┬───────┘
//!        │                                                  │
//!        │ oneshot for order responses                      │ tokio::select!
//!        └─────────────────────────────────────────────────▶│◀── ws_read
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use bulk_client::*;
//! use bulk_client::common::side::Side;
//! use bulk_client::common::tif::TimeInForce;
//! use bulk_client::transaction::TransactionSigner;
//! use bulk_client::parts::WSConfig;
//!
//! #[tokio::main]
//! async fn main() -> eyre::Result<()> {
//!     let signer = TransactionSigner::from_private_key("your_base58_key")?;
//!
//!     let client = BulkWsClient::connect(WSConfig {
//!         url: "wss://exchange-wss.bulk.trade".into(),
//!         symbols: vec!["BTC-USD".into(), "ETH-USD".into()],
//!         signer: Some(signer),
//!         ..Default::default()
//!     }).await?;
//!
//!     // Zero-cost read — no lock, no channel round-trip
//!     if let Some(ticker) = client.get_ticker("BTC-USD") {
//!         println!("BTC mark price: {}", ticker.mark_price);
//!     }
//!
//!     // Place an order — goes through actor → ws
//!     let resp = client.place_limit_order(
//!         "BTC-USD", Side::Buy, 95_000.0, 0.01,
//!         TimeInForce::GTC, false, None, None,
//!     ).await?;
//!
//!     client.shutdown().await;
//!     Ok(())
//! }
//! ```

// ─────────────────────────────────────────────────────────────────────────────
// Topic enum (mirrors topics.py)
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use eyre::bail;
use serde_json::{json, Value};
use crate::msgs::account::{Fill, LeverageSetting, Margin, OrderState, PositionInfo};
use crate::msgs::responses::Response;
use crate::msgs::subscription::SubscriptionRequest;

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitSink;
use serde::Deserialize;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};
use crate::api::parts::command::Command;
use crate::api::parts::config::WSConfig;
use crate::api::parts::{make_nonce, Event, Topic};
use crate::common::side::Side;
use crate::common::tif::TimeInForce;
use crate::msgs::{CancelAll, CancelOrder, LimitOrder, MarketOrder, Price};
use crate::msgs::md::{Candle, L2Snapshot, Ticker};
use crate::transaction::{Action, ActionMeta, Transaction, TransactionSigner};
// ─────────────────────────────────────────────────────────────────────────────
// Snapshot: the full state picture pushed over a single watch channel
// ─────────────────────────────────────────────────────────────────────────────

/// Everything a reader might want, exposed as one cheap clone via `watch`.
/// The actor publishes a new snapshot after every state-mutating message.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(unused)]
pub struct AccountState {
    pub margin: Margin,
    pub positions: HashMap<String, PositionInfo>,
    pub open_orders: HashMap<String, OrderState>,
    pub leverage_settings: HashMap<String, LeverageSetting>,
}

// ═════════════════════════════════════════════════════════════════════════════
// Event callback
// ═════════════════════════════════════════════════════════════════════════════

/// User-supplied callback. Receives the raw JSON payload for the topic.
/// Runs synchronously inside the actor loop — keep it fast or spawn.
#[allow(unused)]
pub type EventHandler = Box<dyn Fn(&Event) + Send + Sync>;

// ═════════════════════════════════════════════════════════════════════════════
// BulkWsClient  —  the public handle (cheap clone, no locks)
// ═════════════════════════════════════════════════════════════════════════════

/// Cloneable client handle.
///
/// - **Hot reads** (ticker, price, margin): `watch::Receiver::borrow()` — zero
///   async overhead, just a ref-counted pointer swap.
/// - **Writes** (subscribe, place orders): go through the `mpsc` command
///   channel to the actor, which serializes all mutations.
/// - **Cold reads** (open orders list): round-trip through the actor via
///   `oneshot` — still fast, but async.
#[allow(unused)]
#[derive(Clone)]
pub struct BulkWsClient {
    // Command channel to the actor
    cmd_tx: mpsc::Sender<Command>,
    // Event handlers
    handlers: Arc<Mutex<HashMap<Topic, Vec<EventHandler>>>>,

    // ── Watch receivers (hot-path, lock-free) ──────────────────────────
    /// Per-symbol ticker snapshots.
    ticker_rx: watch::Receiver<HashMap<String, Ticker>>,
    /// Consolidated account state (margin, positions, orders, leverage).
    account_rx: watch::Receiver<AccountState>,

    // ── Config carried on the handle for convenience ───────────────────
    signer: Option<TransactionSigner>,
    default_timeout: Duration,

    // Monotonic request ID (atomic — no lock needed)
    next_request_id: std::sync::Arc<std::sync::atomic::AtomicU64>,

    // Actor join handle (held in Arc so Clone works)
    actor_handle: std::sync::Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// `true` while the WebSocket actor is running.  Flipped to `false` the
    /// moment the actor begins disconnect teardown.  Cheap sync check, no await.
    connected: Arc<AtomicBool>,

    /// Fires once (with the disconnect reason) when the actor exits.
    /// Clone a receiver via [`BulkWsClient::subscribe_disconnect`] so that
    /// any spawned task can unblock immediately on connection loss.
    disconnect_tx: broadcast::Sender<String>,
}

#[allow(unused)]
impl BulkWsClient {
    // ─────────────────────────────────────────────────────────────────────
    // Construction + connection
    // ─────────────────────────────────────────────────────────────────────

    /// Connect to the exchange and spawn the actor task.
    /// Returns immediately once the WebSocket handshake succeeds.
    ///
    /// # Arguments
    /// - `config`: web socket config
    pub async fn connect(config: WSConfig) -> eyre::Result<Self> {
        info!("Connecting to {}", config.url);
        let (ws_stream, _) = connect_async(&config.url).await?;
        let (ws_write, ws_read) = ws_stream.split();
        info!("Connected to Bulk Exchange WebSocket");

        // Watch channels
        let (ticker_tx, ticker_rx) = watch::channel(HashMap::new());
        let (account_tx, account_rx) = watch::channel(AccountState::default());

        // Command channel (bounded — back-pressure if actor falls behind)
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>(512);

        // Shared handler map between the dispatch task and BulkWsClient (for on() registration)
        let handlers: Arc<Mutex<HashMap<Topic, Vec<EventHandler>>>> = Arc::default();
        let handlers_task = Arc::clone(&handlers);
        let (event_tx, mut event_rx) = mpsc::channel::<(Topic, Event)>(32768);

        tokio::spawn(async move {
            while let Some((topic, event)) = event_rx.recv().await {
                let map = handlers_task.lock().unwrap();
                if let Some(hs) = map.get(&topic) {
                    for h in hs {
                        h(&event);
                    }
                }
            }
        });

        // Build actor
        let connected = Arc::new(AtomicBool::new(true));
        let (disconnect_tx, _) = broadcast::channel::<String>(4);

        let actor = Actor {
            ws_write,
            event_tx,
            cmd_rx,
            ticker_tx,
            account_tx,
            tickers: HashMap::new(),
            prices: HashMap::new(),
            account_state: AccountState::default(),
            pending: HashMap::new(),
            subscriptions: Vec::new(),
            connected: Arc::clone(&connected),
            disconnect_tx: disconnect_tx.clone(),
        };

        // Default subscriptions (same as Python __init__)
        let mut initial_subs = Vec::new();
        if config.track_account {
            if let Some(ref signer) = config.signer {
                let pk_str = signer.public_key_b58();
                initial_subs.push(SubscriptionRequest::new(
                    "account",
                    json!({ "user": pk_str }),
                ));
            }
        }
        if config.track_ticker {
            for sym in &config.symbols {
                initial_subs.push(SubscriptionRequest::new(
                    "ticker",
                    json!({ "symbol": sym }),
                ));
            }
        }

        // Spawn actor
        let actor_handle = tokio::spawn(actor.run(ws_read, initial_subs));

        Ok(Self {
            cmd_tx,
            handlers,
            ticker_rx,
            account_rx,
            signer: config.signer,
            default_timeout: config.default_timeout,
            next_request_id: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
            actor_handle: std::sync::Arc::new(tokio::sync::Mutex::new(Some(actor_handle))),
            connected,
            disconnect_tx,
        })
    }

    /// Shut down the actor and close the WebSocket.
    pub async fn shutdown(&self) {
        let _ = self.cmd_tx.send(Command::Shutdown).await;
        if let Some(h) = self.actor_handle.lock().await.take() {
            let _ = h.await;
        }
    }

    /// Wait for the actor to exit (e.g. on disconnect / error).
    pub async fn closed(&self) {
        if let Some(h) = self.actor_handle.lock().await.take() {
            let _ = h.await;
        }
    }

    /// Returns `true` if the WebSocket actor is still running.
    ///
    /// This is a cheap, lock-free check — safe to call in hot loops.
    /// Once it returns `false` you must call [`BulkWsClient::connect`] again
    /// to establish a new connection.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }


    // ─────────────────────────────────────────────────────────────────────
    // Hot-path reads (zero-cost — no async, no lock)
    // ─────────────────────────────────────────────────────────────────────

    /// Get latest ticker for `symbol`, or `None` if not yet received.
    ///
    /// # Arguments
    /// - `symbol`: symbol to retrieve for
    ///
    /// # Returns
    /// - current ticker if available
    pub fn get_ticker(&self, symbol: &str) -> Option<Ticker> {
        self.ticker_rx.borrow().get(symbol).cloned()
    }

    /// Get Latest mark price for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to retrieve for
    ///
    /// # Returns
    /// - current price if available
    pub fn get_price(&self, symbol: &str) -> Option<f64> {
        self.ticker_rx.borrow().get(symbol).map(|x| x.mark_price)
    }

    /// All current tickers, keyed by symbol.
    ///
    /// # Returns
    /// - all current tickers
    pub fn get_tickers(&self) -> HashMap<String, Ticker> {
        self.ticker_rx.borrow().clone()
    }

    /// Get Current account margin.
    pub fn get_margin(&self) -> Margin {
        self.account_rx.borrow().margin.clone()
    }

    /// Get current position for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to retrieve for
    ///
    /// # Returns
    /// - current position for symbol if available
    pub fn get_position(&self, symbol: &str) -> Option<PositionInfo> {
        self.account_rx.borrow().positions.get(symbol).cloned()
    }

    /// Get all positions.
    pub fn get_positions(&self) -> HashMap<String, PositionInfo> {
        self.account_rx.borrow().positions.clone()
    }

    /// Current leverage setting for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to retrieve for
    ///
    /// # Returns
    /// - current leverage if available
    pub fn get_leverage(&self, symbol: &str) -> Option<f64> {
        self.account_rx
            .borrow()
            .leverage_settings
            .get(symbol)
            .map(|l| l.leverage)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Async reads that wait for changes
    // ─────────────────────────────────────────────────────────────────────

    /// Block until any ticker changes, then return the updated map.
    pub async fn wait_tickers_changed(&mut self) -> eyre::Result<HashMap<String, Ticker>> {
        self.ticker_rx.changed().await?;
        Ok(self.ticker_rx.borrow().clone())
    }

    /// Block until account state changes (margin, positions, orders, leverage).
    pub async fn wait_account_changed(&mut self) -> eyre::Result<AccountState> {
        self.account_rx.changed().await?;
        Ok(self.account_rx.borrow().clone())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Cold reads (round-trip through actor)
    // ─────────────────────────────────────────────────────────────────────

    /// Open orders, optionally filtered by symbol.
    ///
    /// # Arguments
    /// - `symbol`: optional symbol to retrieve for
    ///
    /// # Returns
    /// - current orders or order status
    pub async fn open_orders(&self, symbol: Option<&str>) -> eyre::Result<Vec<OrderState>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Command::GetOrders {
                symbol: symbol.map(Into::into),
                respond: tx,
            })
            .await
            .map_err(|_| eyre::eyre!("actor gone"))?;
        Ok(rx.await?)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Order placement
    // ─────────────────────────────────────────────────────────────────────

    /// Place one or more actions (limit, market, cancel, cancel-all).
    /// Signs the bundle, sends through the actor, and awaits the exchange
    /// response with the configured timeout.
    ///
    /// # Arguments
    /// - `actions`: list of orders, cancels, etc
    /// - `nonce`: nonce to be used
    ///
    /// # Returns
    /// - list of responses
    pub async fn place_orders(
        &self,
        actions: Vec<Action>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Vec<Response>> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let pk = signer.public_key();

        // Build + sign the transaction
        let mut tx = Transaction {
            actions,
            nonce,
            account,
            signer: signer.public_key(),
            signature: Default::default(),
        };
        tx.sign(signer)?;

        let request_id = self
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Build JSON body via tx serialization
        let body = serde_json::to_string(&tx)?;
        let json = format!(
            r#"{{"method":"post","request":{{"type":"action","payload":{}}},"id":{}}}"#,
            body, request_id
        );

        let (resp_tx, resp_rx) = oneshot::channel();

        self.cmd_tx
            .send(Command::Tx {
                request_id,
                json,
                respond: resp_tx,
            })
            .await
            .map_err(|_| eyre::eyre!("client is disconnected — call connect() to reconnect"))?;

        match time::timeout(self.default_timeout, resp_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => bail!("response channel dropped"),
            Err(_) => bail!("order request {request_id} timed out"),
        }
    }

    /// Send oracle price updates and return the exchange's responses.
    ///
    /// Waits for the exchange to acknowledge the transaction so that any
    /// rejection (e.g. invalid price, authorisation failure) is surfaced to
    /// the caller rather than silently dropped.
    ///
    /// # Arguments
    /// - `actions`: list of oracle price updates
    /// - `account`: optional override for the signing account
    /// - `nonce`: optional nonce override; a fresh one is generated when `None`
    ///
    /// # Returns
    /// - `Ok(responses)` — one [`Response`] per submitted price; callers should
    ///   inspect each entry with [`Response::is_error`] to detect rejections.
    /// - `Err(_)` — transport-level failure (send error, timeout, dropped channel).
    pub async fn update_oracle(
        &self,
        actions: Vec<Price>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Vec<Response>> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);

        // Build + sign the transaction
        let mut tx = Transaction {
            actions: actions.iter().map(|a| a.clone().into()).collect(),
            nonce,
            account,
            signer: signer.public_key(),
            signature: Default::default(),
        };
        tx.sign(signer)?;

        let request_id = self
            .next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Build JSON body via tx serialization
        let body = serde_json::to_string(&tx)?;
        let json = format!(
            r#"{{"method":"post","request":{{"type":"action","payload":{}}},"id":{}}}"#,
            body, request_id
        );

        let (resp_tx, resp_rx) = oneshot::channel();

        self.cmd_tx
            .send(Command::Tx {
                request_id,
                json,
                respond: resp_tx,
            })
            .await
            .map_err(|_| eyre::eyre!("client is disconnected — call connect() to reconnect"))?;

        match time::timeout(self.default_timeout, resp_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => bail!("oracle update response channel dropped"),
            Err(_) => bail!("oracle update request {request_id} timed out"),
        }
    }

    // ── Convenience wrappers ─────────────────────────────────────────────

    /// Place limit order
    ///
    /// # Arguments
    /// - `symbol`: which market to execute in
    /// - `side`: buy or sell
    /// - `price`: limit price
    /// - `size`: order size
    /// - `tif`: time in force
    /// - `reduce_only`: true if order is reduce only
    ///
    /// # Returns
    /// - response for order placement
    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: Side,
        price: f64,
        size: f64,
        tif: TimeInForce,
        reduce_only: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let order = LimitOrder {
            symbol: Arc::from(symbol),
            is_buy: side == Side::Buy,
            price,
            size,
            tif,
            reduce_only,
            iso: false,
            commission: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            }
        };
        let resps = self.place_orders(vec![order.into()], None, None).await?;
        resps.into_iter().next().ok_or_else(|| eyre::eyre!("empty response"))
    }

    /// Place market order
    ///
    /// # Arguments
    /// - `symbol`: which market to execute in
    /// - `side`: buy or sell
    /// - `size`: order size
    /// - `reduce_only`: true if order is reduce only
    ///
    /// # Returns
    /// - response for order placement
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: Side,
        size: f64,
        reduce_only: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let order = MarketOrder {
            symbol: Arc::from(symbol),
            is_buy: side == Side::Buy,
            size,
            reduce_only,
            iso: false,
            commission: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            }
        };

        let resps = self.place_orders(vec![order.into()], None, None).await?;
        resps.into_iter().next().ok_or_else(|| eyre::eyre!("empty response"))
    }

    /// Cancel order
    ///
    /// # Arguments
    /// - `symbol`: which market to execute in
    /// - `order_id`: order ID to cancel
    ///
    /// # Returns
    /// - response for order cancel
    pub async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let cancel = CancelOrder {
            symbol: symbol.to_string(),
            oid: Hash::from_str(&order_id)?,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            }
        };

        let resps = self.place_orders(vec![cancel.into()], None, None).await?;
        resps.into_iter().next().ok_or_else(|| eyre::eyre!("empty response"))
    }

    /// Cancel all order
    ///
    /// # Arguments
    /// - `symbols`: which symbols to cancel
    ///
    /// # Returns
    /// - response for order cancel
    pub async fn cancel_all(
        &self, 
        symbols: Vec<String>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let cancel = CancelAll {
            symbols,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            }
        };
        let resps = self.place_orders(vec![cancel.into()], None, None).await?;
        resps.into_iter().next().ok_or_else(|| eyre::eyre!("empty response"))
    }


    // ─────────────────────────────────────────────────────────────────────
    // Subscriptions
    // ─────────────────────────────────────────────────────────────────────

    /// Subscribe to disconnect notifications.
    ///
    /// The returned receiver fires exactly once, carrying the human-readable
    /// disconnect reason, when the actor exits for any reason (server close,
    /// network error, or explicit [`shutdown`]).
    ///
    /// Use this as a *poison pill* for any tasks you spawned that should stop
    /// when the connection is lost:
    ///
    /// ```text
    /// let mut rx = client.subscribe_disconnect();
    /// tokio::spawn(async move {
    ///     let _ = rx.recv().await; // blocks until disconnect
    ///     // clean up your task here
    /// });
    /// ```
    pub fn subscribe_disconnect(&self) -> broadcast::Receiver<String> {
        self.disconnect_tx.subscribe()
    }

    /// Subscribe to ticker for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to subscrive to
    pub async fn subscribe_ticker(&self, symbol: &str) -> eyre::Result<()> {
        self.subscribe(vec![
            SubscriptionRequest::new("ticker", json!({ "symbol": symbol })),
        ]).await
    }

    /// Subscribe to fills for `symbol`.
    ///
    /// # Arguments
    /// - `symbols`: list of symbol to subscribe to
    pub async fn subscribe_trades(&self, symbols: &[&str]) -> eyre::Result<()> {
        let subs = symbols
            .iter()
            .map(|s| SubscriptionRequest::new("trades", json!({ "symbol": s })))
            .collect();
        self.subscribe(subs).await
    }

    /// Subscribe to L2 snapshots for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to subscribe to
    pub async fn subscribe_l2_snapshot(
        &self,
        symbol: &str,
        nlevels: Option<u32>,
    ) -> eyre::Result<()> {
        let mut params = json!({ "symbol": symbol });
        if let Some(n) = nlevels {
            params["nlevels"] = json!(n);
        }
        self.subscribe(vec![SubscriptionRequest::new("l2Snapshot", params)]).await
    }

    /// Subscribe to L2 deltas for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to subscribe to
    pub async fn subscribe_l2_delta(&self, symbol: &str) -> eyre::Result<()> {
        self.subscribe(vec![
            SubscriptionRequest::new("l2Delta", json!({ "symbol": symbol })),
        ]).await
    }

    /// Subscribe to candles for `symbol`.
    ///
    /// # Arguments
    /// - `symbol`: symbol to subscribe to
    /// - `interval`: bar period ("1min", "5min", ...)
    pub async fn subscribe_candles(&self, symbol: &str, interval: &str) -> eyre::Result<()> {
        self.subscribe(vec![SubscriptionRequest::new(
            "candle",
            json!({ "symbol": symbol, "interval": interval }),
        )])
            .await
    }

    /// Subscribe list of subscription requests
    ///
    /// # Arguments
    /// - `subs`: subscription list
    async fn subscribe(&self, subs: Vec<SubscriptionRequest>) -> eyre::Result<()> {
        self.cmd_tx
            .send(Command::Subscribe(subs))
            .await
            .map_err(|_| eyre::eyre!("actor gone"))?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Event handlers
    // ─────────────────────────────────────────────────────────────────────

    /// Register a callback for a topic. The callback runs synchronously
    /// inside the actor loop — keep it fast or `tokio::spawn` from within.
    ///
    /// # Argument
    /// - `topic`: topic to subscribe to
    /// - `handler`: callback for topic
    pub async fn on(&self, topic: Topic, handler: impl Fn(&Event) + Send + Sync + 'static) {
        self.handlers.lock().unwrap().entry(topic).or_default().push(Box::new(handler));
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Actor — owns all mutable state, runs in a single task
// ═════════════════════════════════════════════════════════════════════════════

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
type WsReader = futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

struct Actor {
    // WebSocket write half
    ws_write: WsWriter,
    // Event sender
    event_tx: mpsc::Sender<(Topic,Event)>,

    // Inbound commands from the client handle
    cmd_rx: mpsc::Receiver<Command>,

    // ── Watch senders (actor pushes, clients read) ─────────────────────
    ticker_tx: watch::Sender<HashMap<String, Ticker>>,
    account_tx: watch::Sender<AccountState>,

    // ── Owned state (no locks) ─────────────────────────────────────────
    tickers: HashMap<String, Ticker>,
    prices: HashMap<String, f64>,
    account_state: AccountState,
    pending: HashMap<u64, oneshot::Sender<eyre::Result<Vec<Response>>>>,

    // Subscription log (for reconnection replay)
    subscriptions: Vec<SubscriptionRequest>,

    // ── Disconnect signalling ─────────────────────────────────────────────
    /// Shared flag — flipped to `false` before the `Disconnected` event fires.
    connected: Arc<AtomicBool>,
    /// Broadcast once with the disconnect reason when the actor exits.
    disconnect_tx: broadcast::Sender<String>,
}

impl Actor {
    /// Run loop
    async fn run(
        mut self,
        mut ws_read: WsReader,
        initial_subs: Vec<SubscriptionRequest>,
    ) {
        // Send initial subscriptions
        if !initial_subs.is_empty() {
            if let Err(e) = self.send_subscribe(&initial_subs).await {
                error!("Initial subscription failed: {e}");
                return;
            }
            self.subscriptions = initial_subs;
        }

        // Emit Connected so handlers can react immediately.
        self.emit(Topic::Status, &Event::Connected);

        // Use a labelled loop so every exit path carries an explicit reason.
        let disconnect_reason: String = 'actor: loop {
            tokio::select! {
                // Inbound WebSocket message
                msg = ws_read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            debug!("msg {}: {}", text.len(), &text[0..512.min(text.len())]);
                            match serde_json::from_str::<Value>(&text) {
                                Ok(data) => self.handle_message(data, &text).await,
                                Err(e) => error!("JSON decode error: {e}"),
                            }
                        }
                        Some(Ok(Message::Close(_))) => {
                            warn!("WebSocket closed by server");
                            break 'actor "server closed the connection".into();
                        }
                        Some(Err(e)) => {
                            error!("WebSocket read error: {e}");
                            break 'actor format!("WebSocket read error: {e}");
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            break 'actor "WebSocket stream ended".into();
                        }
                        _ => {} // Ping/Pong handled by tungstenite
                    }
                }

                // Command from client handle
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(Command::Subscribe(subs)) => {
                            if let Err(e) = self.send_subscribe(&subs).await {
                                error!("Subscription send error: {e}");
                            }
                            self.subscriptions.extend(subs);
                        }

                        Some(Command::Tx { request_id, json, respond }) => {
                            self.pending.insert(request_id, respond);
                            if let Err(e) = self.ws_send_text(&json).await {
                                error!("Order send error: {e}");
                                if let Some(tx) = self.pending.remove(&request_id) {
                                    let _ = tx.send(Err(e));
                                }
                            }
                        }

                        Some(Command::AsyncTx { json}) => {
                            if let Err(e) = self.ws_send_text(&json).await {
                                error!("Order send error: {e}");
                            }
                        }

                        Some(Command::SendRaw(json)) => {
                            if let Err(e) = self.ws_send_text(&json).await {
                                error!("Raw send error: {e}");
                            }
                        }

                        Some(Command::GetOrders { symbol, respond }) => {
                            let orders = match symbol {
                                Some(s) => self.account_state.open_orders
                                    .values()
                                    .filter(|o| o.symbol == s)
                                    .cloned()
                                    .collect(),
                                None => self.account_state.open_orders.values().cloned().collect(),
                            };
                            let _ = respond.send(orders);
                        }

                        Some(Command::Shutdown) | None => {
                            info!("Actor shutting down (requested)");
                            break 'actor "shutdown requested".into();
                        }
                    }
                }
            }
        }; // end 'actor loop

        self.handle_disconnect(disconnect_reason).await;
    }

    /// Shared teardown called from every exit path in `run()`.
    ///
    /// Order of operations:
    /// 1. Flip `connected` flag so callers see `is_connected() == false` immediately.
    /// 2. Fail every in-progress `place_orders` that is waiting on a oneshot response.
    /// 3. Emit `Event::Disconnected` on `Topic::Status` so registered handlers fire.
    /// 4. Broadcast the reason string to all `subscribe_disconnect()` receivers
    ///    — this is the poison pill for any other spawned tasks.
    /// 5. Close the WebSocket write half.
    async fn handle_disconnect(&mut self, reason: String) {
        // 1. Mark disconnected — visible to all handles immediately.
        self.connected.store(false, Ordering::Release);

        // 2. Fail every pending order response.
        let err_msg = format!("disconnected: {reason}");
        for (_, tx) in self.pending.drain() {
            let _ = tx.send(Err(eyre::eyre!("{}", err_msg)));
        }

        // 3. Emit the Disconnected event to registered handlers.
        self.emit(Topic::Status, &Event::Disconnected(reason.clone()));

        // 4. Broadcast reason as poison pill (best-effort; ignore no-receivers).
        let _ = self.disconnect_tx.send(reason.clone());

        // 5. Close WS write half (ignore error — connection may already be gone).
        let _ = self.ws_write.close().await;

        info!("Actor stopped: {reason}");
    }

    /// WS send
    async fn ws_send_text(&mut self, text: &str) -> eyre::Result<()> {
        let len = text.len();
        debug!("sending msg len: {}", len);
        self.ws_write
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| eyre::eyre!("ws write: {e}"))?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────
    // Message dispatch
    // ─────────────────────────────────────────────────────────────────────

    async fn handle_message(&mut self, data: Value, json: &str) {
        let msg_type = data["type"].as_str().unwrap_or("");

        match msg_type {
            "subscriptionResponse" => {
                info!(
                    "Subscription confirmed: {:?}",
                    data["topics"].as_array().map(|a| a.len())
                );
            }

            "ticker" => {
                let ticker_v = &data["data"]["ticker"];
                if let Ok(ticker) = serde_json::from_value::<Ticker>(ticker_v.clone()) {
                    self.prices.insert(ticker.symbol.clone(), ticker.mark_price);
                    self.tickers.insert(ticker.symbol.clone(), ticker.clone());

                    // Push watch updates
                    let _ = self.ticker_tx.send(self.tickers.clone());

                    self.emit(Topic::Ticker, &Event::Ticker(ticker.clone()));
                    debug!("Ticker: {} mark={:.2}", ticker.symbol, ticker.mark_price);
                } else {
                    error!("Could not parse ticker event: {:?}", ticker_v);
                }
            }

            "trades" => {
                if let Ok(trades) = serde_json::from_value::<Vec<Fill>>(data["data"].clone()) {
                    self.emit(Topic::Trades, &Event::Trades(trades));
                } else {
                    error!("Could not parse trades event: {:?}", data["data"]);
                }
            }

            "l2Snapshot" => {
                if let Ok(l2_snapshot) = serde_json::from_value::<L2Snapshot>(data["data"]["book"].clone()) {
                    self.emit(Topic::L2Snapshot, &Event::L2Snapshot(l2_snapshot));
                } else {
                    error!("Could not parse l2_snapshot event: msg: {:?}", data["data"]);
                }
            }

            "l2Delta" => {
                if let Ok(l2_delta) = serde_json::from_value::<L2Snapshot>(data["data"]["book"].clone()) {
                    self.emit(Topic::L2Delta, &Event::L2Delta(l2_delta));
                } else {
                    error!("Could not parse l2_delta event: {:?}", data["data"]);
                }
            }

            "candle" => {
                if let Ok(candle) = serde_json::from_value::<Candle>(data["data"].clone()) {
                    self.emit(Topic::Candle, &Event::Candle(candle));
                } else {
                    error!("Could not parse candle event: {:?}", data["data"]);
                }
            }

            "account" => {
                self.handle_account(&data["data"]).await;
            }

            "post" => {
                self.handle_post_response(&data, json);
            }

            other => {
                debug!("Unhandled message type: {other}");
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Account updates
    // ─────────────────────────────────────────────────────────────────────

    async fn handle_account(&mut self, data: &Value) {
        let update_type = data["type"].as_str().unwrap_or("");

        match update_type {
            "accountSnapshot" => {
                if let Ok(margin) = serde_json::from_value::<Margin>(data["margin"].clone()) {
                    self.account_state.margin = margin.clone();
                    self.emit(Topic::Margin, &Event::Margin(margin))
                }

                if let Ok(positions) = serde_json::from_value::<Vec<PositionInfo>>(data["positions"].clone()) {
                    for position in &positions {
                        self.emit(Topic::Position, &Event::Position(position.clone()))
                    }
                    self.account_state.positions = positions
                        .into_iter()
                        .map(|p| (p.symbol.clone(), p.clone()))
                        .collect();
                }

                if let Ok(orders) = serde_json::from_value::<Vec<OrderState>>(data["openOrders"].clone()) {
                    for order in &orders {
                        self.emit(Topic::Order, &Event::Order(order.clone()))
                    }
                    self.account_state.open_orders = orders
                        .into_iter()
                        .map(|o| (o.order_id.clone(), o))
                        .collect();
                }

                if let Ok(leverages) = serde_json::from_value::<Vec<LeverageSetting>>(data["leverageSettings"].clone()) {
                    self.emit(Topic::Leverage, &Event::Leverage(leverages.clone()));
                    for l in leverages {
                        self.account_state.leverage_settings.insert(l.symbol.clone(), l);
                    }
                }

                info!(
                    "Account snapshot: balance={:.2}, positions={}, orders={}",
                    self.account_state.margin.total_balance,
                    self.account_state.positions.len(),
                    self.account_state.open_orders.len(),
                );
            }

            "orderUpdate" => {
                if let Ok(order) = serde_json::from_value::<OrderState>(data.clone()) {
                    let oid = order.order_id.clone();
                    if order.status.is_terminal() {
                        self.account_state.open_orders.remove(&oid);
                    } else {
                        self.account_state.open_orders.insert(oid, order.clone());
                    }
                    self.emit(Topic::Order, &Event::Order(order));
                } else {
                    error!("Could not parse order event: {:?}", data);
                }
            }

            "marginUpdate" => {
                if let Ok(margin) = serde_json::from_value::<Margin>(data.clone()) {
                    self.account_state.margin = margin.clone();
                    self.publish_account();
                    self.emit(Topic::Margin, &Event::Margin(margin));
                } else {
                    error!("Could not parse margin event: {:?}", data);
                }
            }

            "positionUpdate" => {
                if let Ok(pos) = serde_json::from_value::<PositionInfo>(data.clone()) {
                    self.account_state.positions.insert(pos.symbol.clone(), pos.clone());
                    self.emit(Topic::Position, &Event::Position(pos));
                    self.publish_account();
                } else {
                    error!("Could not parse position event: {:?}", data);
                }
            }

            "fill" => {
                if let Ok(fill) = serde_json::from_value::<Fill>(data.clone()) {
                    let dir = fill.side.dir();
                    if let Some(order) = self.account_state.open_orders.get_mut(&fill.order_id) {
                        order.filled_size += fill.size;
                        order.signed_size -= dir * fill.size;
                        if order.signed_size * dir <= 0.0 {
                            self.account_state.open_orders.remove(&fill.order_id);
                        }
                    }
                    self.publish_account();
                    self.emit(Topic::Fill, &Event::Fill(fill.clone()));
                    info!(
                        "Fill: {} {:?} {} @ {} maker={}",
                        fill.symbol, fill.side, fill.size, fill.price, fill.is_maker,
                    );
                } else {
                    error!("Could not parse fill event: {:?}", data);
                }
            }

            "leverageUpdate" => {
                if let Ok(leverages) = serde_json::from_value::<Vec<LeverageSetting>>(data["leverage"].clone()) {
                    for lev in &leverages {
                        self.account_state.leverage_settings.insert(lev.symbol.clone(), lev.clone());
                    }
                    self.emit(Topic::Leverage, &Event::Leverage(leverages.clone()));
                    self.publish_account();
                } else {
                    error!("Could not parse leverage event: {:?}", data);
                }
            }

            _ => {
                debug!("Unknown account update: {update_type}");
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Post (order) response
    // ─────────────────────────────────────────────────────────────────────

    fn handle_post_response(&mut self, data: &Value, _json: &str) {
        let request_id = data["id"].as_u64().unwrap_or(0);
        let inner = &data["data"];
        let rtype = inner["type"].as_str().unwrap_or("");
        let sender = self.pending.remove(&request_id);

        match rtype {
            "action"=> {
                let payload = &inner["payload"];
                let status = payload["status"].as_str().unwrap_or("");

                if status != "ok" {
                    error!("Order request {request_id} failed: {status}");
                    if let Some(tx) = sender {
                        let _ = tx.send(Err(eyre::eyre!("order request failed: {}", data)));
                    }
                    self.emit(Topic::Error, &Event::Error(data.clone()));
                } else {
                    let responses = Response::parse_responses(data);
                    if let Some(tx) = sender {
                        let _ = tx.send(Ok(responses));
                    }
                }
            }
            "ack" => {
                let ok = inner["ok"].as_bool().unwrap_or(false);
                let response = if ok {
                    Response {
                        order_id: None,
                        status: "OK".to_string(),
                        message: None,
                        raw: inner.clone(),
                    }
                } else {
                    let message = inner["message"].as_str().unwrap_or("");
                    Response {
                        order_id: None,
                        status: "Error".to_string(),
                        message: Some(message.to_string()),
                        raw: inner.clone(),
                    }
                };
                if let Some(tx) = sender {
                    let _ = tx.send(Ok(vec![response]));
                }
            }
            _ => panic!("unknown response type: {}", rtype),
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────

    /// Publish the current account state snapshot to the watch channel.
    fn publish_account(&self) {
        let _ = self.account_tx.send(self.account_state.clone());
    }

    /// Fire all handlers registered for `topic`.
    fn emit(&self, topic: Topic, data: &Event) {
        let _ = self.event_tx.try_send((topic, data.clone()));
    }

    /// Send a JSON value over the WebSocket.
    async fn ws_send_json(&mut self, value: &Value) -> eyre::Result<()> {
        let text = serde_json::to_string(value)?;
        self.ws_write
            .send(Message::Text(text.into()))
            .await
            .map_err(|e| eyre::eyre!("ws write: {e}"))?;
        Ok(())
    }

    /// Send subscription request(s) over the WebSocket.
    async fn send_subscribe(&mut self, subs: &[SubscriptionRequest]) -> eyre::Result<()> {
        let request = json!({
            "method": "subscribe",
            "subscription": subs.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
        });
        self.ws_send_json(&request).await?;
        info!("Subscribed to {} topics", subs.len());
        Ok(())
    }
}
