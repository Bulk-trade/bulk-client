//! ═══════════════════════════════════════════════════════════════════════════
//! Mock Bulk Exchange WebSocket Server
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! A minimal fake exchange server that speaks the Bulk Labs WebSocket protocol,
//! sufficient to exercise the RandomMarketMaker client end-to-end.
//!
//! Supported inbound messages:
//!   • `subscribe`  — confirms subscriptions, sends account snapshot + ticker stream
//!   • `post` with `action.type == "order"` — returns `ok` with `resting` per action
//!   • `post` with `action.type == "oracle"` — returns `ok` (fire-and-forget on client)
//!
//! All responses are structurally correct but contain faked/default data.

use clap::Parser;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// How often to push ticker updates (ms)
const TICKER_INTERVAL_MS: u64 = 1000;

// ─────────────────────────────────────────────────────────────────────────────
// Counters (for fake order-ID generation)
// ─────────────────────────────────────────────────────────────────────────────

static ORDER_SEQ: AtomicU64 = AtomicU64::new(1);

fn next_fake_oid() -> String {
    let seq = ORDER_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("mock-oid-{seq:012}")
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-connection handler
// ─────────────────────────────────────────────────────────────────────────────

/// Outbound messages queued by handlers and the ticker task.
type OutboundTx = mpsc::UnboundedSender<Message>;

/// Tracks per-connection subscription state.
struct ConnState {
    /// Symbols with active ticker subscriptions.
    ticker_symbols: HashSet<String>,
    /// Whether an account subscription is active.
    has_account_sub: bool,
    /// Running count of inbound messages (for logging).
    msg_count: u64,
    /// Running count of orders received.
    order_count: u64,
    /// Running count of oracle updates received.
    oracle_count: u64,
}

impl ConnState {
    fn new() -> Self {
        Self {
            ticker_symbols: HashSet::new(),
            has_account_sub: false,
            msg_count: 0,
            order_count: 0,
            oracle_count: 0,
        }
    }
}

async fn handle_connection(stream: TcpStream) -> eyre::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    // Channel for outbound messages (handlers + ticker task → writer)
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();

    let mut state = ConnState::new();

    // Shared flag so ticker task knows about subscribed symbols
    let ticker_symbols: Arc<tokio::sync::Mutex<HashSet<String>>> =
        Arc::new(tokio::sync::Mutex::new(HashSet::new()));

    // Spawn ticker broadcast task
    let ticker_syms = ticker_symbols.clone();
    let ticker_tx = out_tx.clone();
    let ticker_task = tokio::spawn(async move {
        ticker_loop(ticker_tx, ticker_syms).await;
    });

    loop {
        tokio::select! {
            // ── Inbound from client ──────────────────────────────────────
            msg = ws_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        state.msg_count += 1;
                        if let Err(e) = handle_inbound(
                            &text, &mut state, &out_tx, &ticker_symbols
                        ).await {
                            warn!("Error handling message: {e}");
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = out_tx.send(Message::Pong(data));
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        error!("WS read error: {e}");
                        break;
                    }
                    _ => {}
                }
            }

            // ── Outbound to client ───────────────────────────────────────
            Some(msg) = out_rx.recv() => {
                if let Err(e) = ws_write.send(msg).await {
                    error!("WS write error: {e}");
                    break;
                }
            }
        }
    }

    ticker_task.abort();
    let _ = ws_write.close().await;
    info!(
        "Session summary: msgs={} orders={} oracles={}",
        state.msg_count, state.order_count, state.oracle_count
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Inbound message dispatch
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_inbound(
    text: &str,
    state: &mut ConnState,
    out_tx: &OutboundTx,
    ticker_symbols: &Arc<tokio::sync::Mutex<HashSet<String>>>,
) -> eyre::Result<()> {
    let data: Value = serde_json::from_str(text)?;
    let method = data["method"].as_str().unwrap_or("");

    match method {
        "subscribe" => {
            handle_subscribe(&data, state, out_tx, ticker_symbols).await?;
        }
        "post" => {
            handle_post(&data, state, out_tx)?;
        }
        other => {
            debug!("Unknown method: {other:?} (msg #{})", state.msg_count);
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Subscribe handler
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_subscribe(
    data: &Value,
    state: &mut ConnState,
    out_tx: &OutboundTx,
    ticker_symbols: &Arc<tokio::sync::Mutex<HashSet<String>>>,
) -> eyre::Result<()> {
    let subs = data["subscription"].as_array();
    let mut topics = Vec::new();

    if let Some(subs) = subs {
        for sub in subs {
            let sub_type = sub["type"].as_str().unwrap_or("");
            match sub_type {
                "ticker" => {
                    if let Some(symbol) = sub["symbol"].as_str() {
                        state.ticker_symbols.insert(symbol.to_string());
                        ticker_symbols.lock().await.insert(symbol.to_string());
                        topics.push(format!("ticker.{symbol}"));
                        info!("Subscribed to ticker: {symbol}");
                    }
                }
                "account" => {
                    let user = sub["user"].as_str().unwrap_or("unknown");
                    state.has_account_sub = true;
                    topics.push(format!("account.{user}"));
                    info!("Subscribed to account: {user}");

                    // Send account snapshot
                    let snapshot = make_account_snapshot(user);
                    out_tx.send(Message::Text(snapshot.to_string().into()))?;
                    info!("Sent account snapshot");
                }
                "trades" | "l2Snapshot" | "l2Delta" | "candle" => {
                    let symbol = sub["symbol"].as_str().unwrap_or("?");
                    topics.push(format!("{sub_type}.{symbol}"));
                    info!("Subscribed to {sub_type}: {symbol} (no-op)");
                }
                _ => {
                    debug!("Unknown subscription type: {sub_type}");
                }
            }
        }
    }

    // Send subscription confirmation
    let confirmation = json!({
        "type": "subscriptionResponse",
        "topics": topics,
    });
    out_tx.send(Message::Text(confirmation.to_string().into()))?;
    debug!("Sent subscription confirmation for {} topics", topics.len());

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Post handler (orders + oracle)
// ─────────────────────────────────────────────────────────────────────────────

fn handle_post(
    data: &Value,
    state: &mut ConnState,
    out_tx: &OutboundTx,
) -> eyre::Result<()> {
    let request_id = data["id"].as_u64().unwrap_or(0);
    let payload = &data["request"]["payload"];
    let action_type = payload["action"]["type"].as_str().unwrap_or("");

    match action_type {
        "order" => {
            let orders = payload["action"]["orders"].as_array();
            let n_actions = orders.map(|a| a.len()).unwrap_or(0);
            state.order_count += n_actions as u64;

            // Build one status entry per action
            let statuses: Vec<Value> = (0..n_actions)
                .map(|_| {
                    let oid = next_fake_oid();
                    json!({ "resting": { "oid": oid } })
                })
                .collect();

            let response = make_post_response(request_id, &statuses);
            out_tx.send(Message::Text(response.to_string().into()))?;

            if state.order_count % 500 == 0 {
                info!(
                    "Order batch: id={request_id} actions={n_actions} (total={})",
                    state.order_count
                );
            } else {
                debug!(
                    "Order batch: id={request_id} actions={n_actions} (total={})",
                    state.order_count
                );
            }
        }

        "oracle" => {
            let prices = payload["action"]["oracles"].as_array();
            let n_prices = prices.map(|a| a.len()).unwrap_or(0);
            state.oracle_count += n_prices as u64;

            // Oracle is fire-and-forget from the client (SendRaw), but we still
            // return a valid post response in case anything checks.
            let statuses = vec![json!({ "ok": {} })];
            let response = make_post_response(request_id, &statuses);
            out_tx.send(Message::Text(response.to_string().into()))?;

            debug!(
                "Oracle update: id={request_id} prices={n_prices} (total={})",
                state.oracle_count
            );
        }

        _ => {
            warn!("Unknown action type in post: {action_type:?} (id={request_id})");
            // Return an error response
            let response = json!({
                "type": "post",
                "id": request_id,
                "data": {
                    "payload": {
                        "status": "error",
                        "message": format!("unknown action type: {action_type}"),
                    }
                }
            });
            out_tx.send(Message::Text(response.to_string().into()))?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Response builders
// ─────────────────────────────────────────────────────────────────────────────

/// Build the post response envelope the client's `handle_post_response` expects.
///
/// Path the client reads:
///   data["id"]                                     → request_id
///   data["data"]["payload"]["status"]               → "ok"
///   data["data"]["payload"]["response"]["data"]["statuses"] → [{...}, ...]
fn make_post_response(request_id: u64, statuses: &[Value]) -> Value {
    json!({
        "type": "post",
        "id": request_id,
        "data": {
            "payload": {
                "status": "ok",
                "response": {
                    "data": {
                        "statuses": statuses
                    }
                }
            }
        }
    })
}

/// Build an account snapshot that satisfies the client's `handle_account` parser.
///
/// Path the client reads:
///   data["type"]           → "account"
///   data["data"]["type"]   → "accountSnapshot"
///   data["data"]["margin"] → Margin { ... }
///   data["data"]["positions"]        → []
///   data["data"]["openOrders"]       → []
///   data["data"]["leverageSettings"] → []
fn make_account_snapshot(user: &str) -> Value {
    json!({
        "type": "account",
        "data": {
            "type": "accountSnapshot",
            "margin": {
                "totalBalance": 1_000_000.0,
                "availableBalance": 1_000_000.0,
                "marginUsed": 0.0,
                "notional": 0.0,
                "realizedPnl": 0.0,
                "unrealizedPnl": 0.0,
                "fees": 0.0,
                "funding": 0.0
            },
            "positions": [],
            "openOrders": [],
            "leverageSettings": []
        },
        "topic": format!("account.{user}")
    })
}

/// Build a ticker message for the given symbol at `price`.
fn make_ticker(symbol: &str, price: f64) -> Value {
    json!({
        "type": "ticker",
        "data": {
            "ticker": {
                "symbol": symbol,
                "lastPrice": price,
                "markPrice": price,
                "oraclePrice": price,
                "priceChange": 0.0,
                "priceChangePercent": 0.0,
                "highPrice": price * 1.01,
                "lowPrice": price * 0.99,
                "volume": 12345.6,
                "quoteVolume": 1_234_567.89,
                "openInterest": 500.0,
                "fundingRate": 0.0001
            }
        }
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Ticker broadcast loop
// ─────────────────────────────────────────────────────────────────────────────

/// Periodically pushes ticker updates for all subscribed symbols.
async fn ticker_loop(
    out_tx: OutboundTx,
    symbols: Arc<tokio::sync::Mutex<HashSet<String>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(TICKER_INTERVAL_MS));
    let mut tick: u64 = 0;

    loop {
        interval.tick().await;
        tick += 1;

        let syms: Vec<String> = symbols.lock().await.iter().cloned().collect();
        if syms.is_empty() {
            continue;
        }

        for sym in &syms {
            // Generate a slowly drifting price based on symbol
            let base = default_price(sym);
            let noise = (tick as f64 * 0.01).sin() * base * 0.001;
            let price = base + noise;

            let ticker = make_ticker(sym, price);
            if out_tx.send(Message::Text(ticker.to_string().into())).is_err() {
                return; // connection closed
            }
        }
    }
}

/// Default base prices for known symbols.
fn default_price(symbol: &str) -> f64 {
    match symbol {
        "BTC-USD" => 100_000.0,
        "ETH-USD" => 3_500.0,
        "SOL-USD" => 180.0,
        _ => 1000.0,
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════

/// OU-Process Random Market Maker (performance testing)
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// server port
    #[arg(long, default_value = "127.0.0.1:9090")]
    url: String,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Entry point
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| cli.log_level.clone().into()),
        )
        .init();

    let addr: SocketAddr = SocketAddr::from_str(cli.url.as_str())?;

    let listener = TcpListener::bind(addr).await?;
    info!("Mock exchange listening on ws://{addr}");

    loop {
        let (stream, peer) = listener.accept().await?;
        info!("New connection from {peer}");
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                error!("Connection error ({peer}): {e}");
            }
            info!("Connection closed ({peer})");
        });
    }
}