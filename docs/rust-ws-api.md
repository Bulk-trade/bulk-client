# `BulkWsClient` — WebSocket API Reference

`BulkWsClient` provides a persistent, event-driven connection to the Bulk exchange.
Unlike the HTTP client, it maintains live local state that can be read synchronously
without round-trips:

- **Market data** — ticker prices, L2 snapshots and deltas, trades, candles
- **Account state** — margin, positions, open orders, leverage (auto-subscribed when a signer is present)
- **Trading** — signed transactions sent over the socket with per-response callbacks

---

## 1. Instantiating `BulkWsClient`

All construction goes through `BulkWsClient::connect(config)`, which completes the
WebSocket handshake before returning a handle.

### `WSConfig` fields

| Field | Type | Default | Description |
|---|---|---|---|
| `url` | `String` | — | WebSocket endpoint (`wss://...`) |
| `symbols` | `Vec<String>` | `[]` | Symbols to auto-subscribe tickers for |
| `signer` | `Option<TransactionSigner>` | `None` | Required for any trading operation |
| `track_account` | `bool` | `true` | Auto-subscribe to account stream if signer present |
| `track_ticker` | `bool` | `true` | Auto-subscribe to tickers for `symbols` |
| `default_timeout` | `Duration` | 5 s | Timeout applied to every order response |

### Without a signer — market data only

No private key is needed for read-only access. Omit the `signer` field (it defaults
to `None`). Attempts to call any trading method on a signer-less client will return
an `Err` immediately.

```rust
use bulk_sdk::{BulkWsClient, WSConfig};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkWsClient::connect(WSConfig {
        url: "wss://exchange-wss.bulk.trade".into(),
        symbols: vec!["BTC-USD".into(), "ETH-USD".into()],
        ..Default::default()    // signer: None
    })
    .await?;

    Ok(())
}
```

### With a signer — trading enabled

Pass a `TransactionSigner` built from a base-58-encoded private key. The client
will auto-subscribe to the account stream (margin, positions, open orders, leverage)
alongside the tickers for `symbols`.

```rust
use bulk_sdk::{BulkWsClient, WSConfig, TransactionSigner};
use std::time::Duration;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let signer = TransactionSigner::from_private_key("YOUR_BASE58_PRIVATE_KEY")?;

    let client = BulkWsClient::connect(WSConfig {
        url: "wss://exchange-wss.bulk.trade".into(),
        symbols: vec!["BTC-USD".into(), "SOL-USD".into()],
        signer: Some(signer),
        default_timeout: Duration::from_secs(10),
        ..Default::default()
    })
    .await?;

    println!("Connected. Balance: {:.2}", client.get_margin().total_balance);

    client.shutdown().await;
    Ok(())
}
```

### Connection lifecycle helpers

```rust
// Cheap, lock-free — safe in hot loops
if !client.is_connected() {
    // reconnect logic
}

// Await actor exit (e.g. server closed the socket)
client.closed().await;

// Graceful shutdown
client.shutdown().await;

// React to disconnect from a spawned task
let mut disc_rx = client.subscribe_disconnect();
tokio::spawn(async move {
    if let Ok(reason) = disc_rx.recv().await {
        eprintln!("Lost connection: {reason}");
    }
});
```

---

## 2. Transactions — Actions and Batching

Every trading operation on the exchange is expressed as a **transaction**
(`Transaction`) that carries one or more **actions** (`Action`). This mirrors
the exchange wire protocol: actions are serialised together, the nonce and
account are appended, and the whole payload is signed once.

### The `Action` enum

```
Action::LimitOrder(LimitOrder)       // Passive limit order
Action::MarketOrder(MarketOrder)     // Aggressive market order
Action::ModifyOrder(ModifyOrder)     // Amend price/size in-place
Action::Cancel(CancelOrder)          // Cancel by order ID
Action::CancelAll(CancelAll)         // Cancel all orders, optionally filtered by symbol(s)
Action::Stop(StopOrTP)               // Stop-loss conditional
Action::TakeProfit(StopOrTP)         // Take-profit conditional
Action::Trailing(Trailing)           // Trailing stop
Action::Range(Range)                 // Range conditional order
Action::Trigger(Trigger)             // Generic trigger
Action::OnFill(OnFill)               // Post-fill action
// ... and admin actions (oracle updates, sub-accounts, multisig, etc.)
```

Every concrete action type (e.g. `LimitOrder`) has an embedded `ActionMeta`
carrying `account`, `nonce`, `seqno`, and an optional cached hash. When you use
the convenience wrappers on `BulkWsClient` (§3), the client populates `meta`
for you automatically.

### Single-action transactions

The convenience wrappers each wrap one action in its own transaction:

```rust
// Each of these is its own signed transaction, sent individually.
client.place_limit_order("BTC-USD", Side::Buy, 95_000.0, 0.1,
                          TimeInForce::GTC, false, None, None).await?;
client.cancel_order("BTC-USD", &order_id, None, None).await?;
```

### Multi-action transactions (batching)

`place_tx(actions, account, nonce)` lets you bundle multiple `Action` values into
a single signed transaction. The exchange processes them atomically in order, and
you receive one `Response` per action back.

This is particularly useful for:

- **Ladder entry**: placing several limit orders at different price levels in one round-trip.
- **Atomic replace**: cancelling an existing order and placing a new one in the same
  transaction, with no window where neither order is live.

```rust
use bulk_sdk::{Action, LimitOrder, CancelOrder, ActionMeta, Side, TimeInForce};
use std::sync::Arc;

// --- Ladder: three limit buys at different price levels ---
let actions: Vec<Action> = [94_000.0, 93_000.0, 92_000.0]
    .iter()
    .map(|&price| {
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price,
            size: 0.05,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            meta: ActionMeta::default(),  // client fills this in before signing
        })
    })
    .collect();

let responses = client.place_tx(actions, None, None).await?;
for resp in &responses {
    println!("order_id={:?} status={}", resp.order_id, resp.status);
}
```

```rust
// --- Atomic replace: cancel + re-place in one tx ---
use std::str::FromStr;
use solana_hash::Hash;

let cancel = Action::Cancel(CancelOrder {
    symbol: "BTC-USD".to_string(),
    oid: Hash::from_str(&existing_order_id)?,
    meta: ActionMeta::default(),
});

let new_order = Action::LimitOrder(LimitOrder {
    symbol: Arc::from("BTC-USD"),
    is_buy: true,
    price: 94_500.0,
    size: 0.1,
    tif: TimeInForce::GTC,
    reduce_only: false,
    iso: false,
    meta: ActionMeta::default(),
});

// Both actions land in the same signed transaction.
let responses = client.place_tx(vec![cancel, new_order], None, None).await?;
```

### Optional `account` and `nonce` parameters

All trading methods accept `account: Option<Pubkey>` and `nonce: Option<u64>`:

- **`account`**: override the target account (e.g. when operating as an agent wallet
  on behalf of a sub-account). Defaults to the signer's own public key.
- **`nonce`**: supply a deterministic nonce (useful in tests or for cross-system
  coordination). Defaults to a monotonic timestamp-derived value.

Pass `None` for both in normal usage.

---

## 3. Executing Transactions — Examples

### Convenience wrappers

These are thin wrappers around `place_tx` for the most common cases. Each builds
exactly one action, sends it, and returns the single `Response`.

#### Limit order

```rust
use bulk_sdk::{Side, TimeInForce};

let resp = client
    .place_limit_order(
        "BTC-USD",          // symbol
        Side::Buy,          // side
        95_000.0,           // limit price
        0.1,                // size
        TimeInForce::GTC,   // GTC | IOC | ALO (add-liquidity-only)
        false,              // reduce_only
        None,               // account (default: own pubkey)
        None,               // nonce   (default: auto)
    )
    .await?;

println!("placed order_id={:?}", resp.order_id);
```

#### Market order

```rust
let resp = client
    .place_market_order("ETH-USD", Side::Sell, 1.0, false, None, None)
    .await?;
```

#### Cancel a specific order

```rust
let resp = client
    .cancel_order("BTC-USD", &order_id, None, None)
    .await?;
```

#### Cancel all orders (optionally filtered)

```rust
// Cancel all open orders across every symbol.
let resp = client.cancel_all(vec![], None, None).await?;

// Cancel only BTC-USD and ETH-USD orders.
let resp = client
    .cancel_all(vec!["BTC-USD".into(), "ETH-USD".into()], None, None)
    .await?;
```

### Reading account state

All reads below are synchronous (no `.await`) and lock-free — safe to call
inside tight loops.

```rust
// Account margin / collateral
let margin = client.get_margin();
println!("balance={:.2}  available={:.2}", margin.total_balance, margin.available_balance);

// Per-symbol position
if let Some(pos) = client.get_position("BTC-USD") {
    println!("BTC position: size={} entry={}", pos.signed_size, pos.entry_price);
}

// All positions
let positions = client.get_positions();

// Current leverage for a symbol
if let Some(lev) = client.get_leverage("BTC-USD") {
    println!("BTC leverage: {lev}x");
}

// Open orders — async round-trip through actor
let orders = client.open_orders(Some("BTC-USD")).await?;
let all_orders = client.open_orders(None).await?;
```

### Waiting for state changes

When you want to react to the *next* account or ticker change rather than polling:

```rust
// Block until any ticker changes, then process the snapshot.
let mut client_mut = client.clone();
loop {
    let tickers = client_mut.wait_tickers_changed().await?;
    if let Some(t) = tickers.get("BTC-USD") {
        println!("BTC updated: mark={}", t.mark_price);
    }
}

// Block until account state changes (fills, margin updates, etc.)
let account = client_mut.wait_account_changed().await?;
println!("new balance: {:.2}", account.margin.total_balance);
```

---

## 4. Subscribing to Market Data

### Auto-subscriptions on connect

When `track_ticker = true` (the default), the client automatically subscribes
to the **ticker** stream for every symbol listed in `WSConfig::symbols`. No
extra call is needed — `get_ticker()` / `get_price()` are ready as soon as the
first server message arrives.

### Subscribing dynamically after connect

#### Ticker

```rust
// Subscribe to a ticker not in the initial symbol list.
client.subscribe_ticker("SOL-USD").await?;

// Synchronous read after subscription arrives.
if let Some(ticker) = client.get_ticker("SOL-USD") {
    println!("SOL mark price: {}", ticker.mark_price);
}

// Or get the mark price directly.
if let Some(price) = client.get_price("SOL-USD") {
    println!("SOL price: {price}");
}

// Snapshot of all subscribed tickers.
let all = client.get_tickers();
```

#### L2 order book

```rust
// Full snapshot — optional depth limit.
client.subscribe_l2_snapshot("BTC-USD", Some(20)).await?;

// Incremental deltas (subscribe after the snapshot to avoid gaps).
client.subscribe_l2_delta("BTC-USD").await?;
```

#### Trades and candles

```rust
// Public trade feed for one or more symbols.
client.subscribe_trades(&["BTC-USD", "ETH-USD"]).await?;

// OHLCV candles — interval strings: "1min", "5min", "15min", "1h", "4h", "1d"
client.subscribe_candles("BTC-USD", "5min").await?;
```

### Event callbacks with `on()`

Register a callback for any `Topic`. The closure runs synchronously inside the
actor loop — keep it fast (< 1 ms), or spawn a new task for heavier work.

```rust
use bulk_sdk::{Topic, Event};

// React to every ticker update.
client.on(Topic::Ticker, |event| {
    if let Event::Ticker(t) = event {
        println!("[ticker] {} mark={}", t.symbol, t.mark_price);
    }
}).await;

// React to fills.
client.on(Topic::Fill, |event| {
    if let Event::Fill(fill) = event {
        println!("[fill] {} {:?} {} @ {}", fill.symbol, fill.side, fill.size, fill.price);
    }
}).await;

// React to order lifecycle events.
client.on(Topic::Order, |event| {
    if let Event::Order(order) = event {
        println!("[order] {} status={:?}", order.order_id, order.status);
    }
}).await;

// React to margin / position updates.
client.on(Topic::Margin,   |ev| { /* ... */ }).await;
client.on(Topic::Position, |ev| { /* ... */ }).await;

// React to connection / disconnection.
client.on(Topic::Status, |event| {
    match event {
        Event::Connected    => println!("WebSocket connected"),
        Event::Disconnected => println!("WebSocket disconnected"),
        _ => {}
    }
}).await;
```

### `Topic` / `Event` reference

| `Topic` | `Event` variant | Description |
|---|---|---|
| `Topic::Ticker` | `Event::Ticker(Ticker)` | Mark price, last price, funding rate, volume |
| `Topic::Fill` | `Event::Fill(Fill)` | Individual trade fill for the account |
| `Topic::Order` | `Event::Order(OrderState)` | Order lifecycle update (placed, cancelled, filled) |
| `Topic::Margin` | `Event::Margin(Margin)` | Account balance and available margin change |
| `Topic::Position` | `Event::Position(Position)` | Open position update |
| `Topic::Leverage` | `Event::Leverage(Leverage)` | Per-symbol leverage setting change |
| `Topic::L2Snapshot` | `Event::L2Snapshot(L2Book)` | Full order book snapshot |
| `Topic::L2Delta` | `Event::L2Delta(L2Delta)` | Incremental order book update |
| `Topic::Trade` | `Event::Trade(Trade)` | Public trade feed |
| `Topic::Candle` | `Event::Candle(Candle)` | OHLCV candle update |
| `Topic::Error` | `Event::Error(String)` | Exchange or protocol error message |
| `Topic::Status` | `Event::Connected` / `Event::Disconnected` | Connection state change |

---

## 5. The `Response` Type

Every trading method returns a `Response` (or `Vec<Response>` for `place_tx`).

```rust
pub struct Response {
    pub order_id: Option<String>,  // present for resting/working/filled placements
    pub status:   String,          // see status strings below
    pub message:  Option<String>,  // error detail when status == "error"
    pub raw:      Value,           // raw JSON from the exchange
}
```

### Status strings

| Status | Meaning |
|---|---|
| `"resting"` | Limit order is live on the book |
| `"working"` | Order is being processed |
| `"filled"` | Order was fully filled immediately |
| `"cancelled"` | Cancel accepted |
| `"error"` | Generic rejection — check `message` for detail |
| `"rejectedRiskLimit"` | Rejected: would exceed risk/leverage limits |
| `"rejectedInvalid"` | Rejected: malformed or invalid parameters |
| `"rejectedDuplicate"` | Rejected: duplicate order ID |
| `"rejectedCrossing"` | Rejected: would cross own orders |

### Helper methods

```rust
if resp.is_error() {
    eprintln!("order rejected: {:?}", resp.message);
}

if resp.is_placement() {
    println!("order live: oid={:?}", resp.order_id);
}
```

---

## 6. Full Example — Combining Market Data and Trading

```rust
use bulk_sdk::{BulkWsClient, WSConfig, TransactionSigner, Topic, Event, Side, TimeInForce};
use std::time::Duration;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let signer = TransactionSigner::from_private_key("YOUR_BASE58_PRIVATE_KEY")?;

    let client = BulkWsClient::connect(WSConfig {
        url: "wss://exchange-wss.bulk.trade".into(),
        symbols: vec!["BTC-USD".into()],
        signer: Some(signer),
        ..Default::default()
    })
    .await?;

    // Subscribe to book data
    client.subscribe_l2_snapshot("BTC-USD", Some(5)).await?;
    client.subscribe_l2_delta("BTC-USD").await?;

    // Register an async-friendly handler: spawn per-event to avoid blocking actor.
    let worker = client.clone();
    client.on(Topic::Ticker, move |event| {
        if let Event::Ticker(t) = event {
            let w = worker.clone();
            let mark = t.mark_price;
            tokio::spawn(async move {
                // Place a bid 10 bps below mid.
                let bid = mark * 0.999;
                if let Err(e) = w.place_limit_order(
                    "BTC-USD", Side::Buy, bid, 0.01,
                    TimeInForce::ALO, false, None, None,
                ).await {
                    eprintln!("order error: {e}");
                }
            });
        }
    }).await;

    // Disconnect poison pill for the main task.
    let mut disc = client.subscribe_disconnect();
    disc.recv().await.ok();

    println!("Disconnected — exiting.");
    Ok(())
}
```