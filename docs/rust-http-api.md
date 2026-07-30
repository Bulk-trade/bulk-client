# `BulkHttpClient` — HTTP REST API Reference

`BulkHttpClient` provides synchronous request/response access to the Bulk exchange over HTTP.
It covers three categories of endpoints:

- **Market data** — public, unsigned (tickers, order books, candles)
- **Account queries** — public, unsigned (account state, open orders, paginated history)
- **Trading & settings** — private, signed (orders, cancels, leverage, agent wallets)

---

## 1. Instantiating `BulkHttpClient`

### `HttpConfig` fields

| Field | Type | Default | Description |
|---|---|---|---|
| `base_url` | `String` | — | HTTP endpoint (`https://...`) |
| `signer` | `Option<TransactionSigner>` | `None` | Required for any trading or settings operation |
| `signature_domain` | `Option<SignatureDomain>` | `None` | Required when `signer` is set; selects mainnet, testnet, or devnet |
| `default_timeout` | `Duration` | 10 s | Per-request timeout applied to every call |

The configured domain binds every authenticated signature. Binary transactions
append its one-byte value to the signable bytes; generic canonical-JSON
signatures append the same byte, while Ledger clear-sign envelopes place it in
the first application-domain byte. The domain is not sent as a request field.

### `BulkHttpClient::new(config)` — from `HttpConfig`

```rust
use bulk_sdk::{BulkHttpClient, HttpConfig, SignatureDomain, TransactionSigner};
use std::time::Duration;

// Read-only (market data + account queries)
let config = HttpConfig {
    base_url: "https://exchange.bulk.trade".into(),
    signer: None,
    signature_domain: None,
    default_timeout: Duration::from_secs(10),
};
let client = BulkHttpClient::new(&config)?;
```

```rust
// Authenticated (trading enabled)
let signer = TransactionSigner::from_private_key("YOUR_BASE58_PRIVATE_KEY")?;
let config = HttpConfig {
    base_url: "https://exchange.bulk.trade".into(),
    signer: Some(signer),
    signature_domain: Some(SignatureDomain::Mainnet),
    default_timeout: Duration::from_secs(10),
};
let client = BulkHttpClient::new(&config)?;
```

### `BulkHttpClient::with_url(base_url, private_key, signature_domain)` — convenience constructor

Builds an `HttpConfig` internally with a 10-second default timeout.

```rust
// Read-only
let client = BulkHttpClient::with_url("https://exchange.bulk.trade", None, None)?;

// Authenticated
let client = BulkHttpClient::with_url(
    "https://exchange.bulk.trade",
    Some("YOUR_BASE58_PRIVATE_KEY"),
    Some(SignatureDomain::Mainnet),
)?;
```

### Accessor helpers

```rust
// Retrieve the underlying HttpConfig
let cfg = client.config();

// Retrieve the signer's public key (None if no signer was provided)
if let Some(pk) = client.public_key() {
    println!("trading as: {pk}");
}
```

---

## 2. Market Data Endpoints (unsigned)

All market data endpoints are public — no private key required.

### `get_exchange_info()`

Returns metadata for all available markets.

```rust
let markets: Vec<MarketInfo> = client.get_exchange_info().await?;
for m in &markets {
    println!("{}: tick={} lot={}", m.symbol, m.tick_size, m.lot_size);
}
```

### `get_ticker(symbol)`

Returns the current ticker/statistics for a single market.

```rust
let ticker: Ticker = client.get_ticker("BTC-USD").await?;
println!("mark={} last={} funding={:.6} vol={}",
    ticker.mark_price, ticker.last_price, ticker.funding_rate, ticker.volume);
```

### `get_klines(symbol, interval, start_time, end_time, limit)`

Returns historical OHLCV candles.

| Argument | Type | Description |
|---|---|---|
| `symbol` | `&str` | Market symbol, e.g. `"BTC-USD"` |
| `interval` | `&str` | Candle width: `"1m"` `"5m"` `"15m"` `"30m"` `"1h"` `"4h"` `"1d"` `"1w"` |
| `start_time` | `Option<u64>` | Start timestamp in milliseconds |
| `end_time` | `Option<u64>` | End timestamp in milliseconds |
| `limit` | `Option<u32>` | Max candles to return (default `500`, max `1000`) |

```rust
// Last 100 hourly candles for BTC-USD
let candles: Vec<Candle> = client
    .get_klines("BTC-USD", "1h", None, None, Some(100))
    .await?;

for c in &candles {
    println!("t={} o={} h={} l={} c={} v={}", c.time, c.open, c.high, c.low, c.close, c.volume);
}
```

### `get_orderbook(symbol, nlevels, aggregation)`

Returns an L2 order book snapshot.

| Argument | Type | Description |
|---|---|---|
| `symbol` | `&str` | Market symbol |
| `nlevels` | `Option<u32>` | Price levels per side (default `20`, max `1000`) |
| `aggregation` | `Option<f64>` | Optional price grouping/bucketing |

```rust
let book: L2Snapshot = client.get_orderbook("BTC-USD", Some(10), None).await?;

let (bids, asks) = &book.levels;
let best_bid = bids.first().map(|l| l.price).unwrap_or(0.0);
let best_ask = asks.first().map(|l| l.price).unwrap_or(0.0);
println!("spread: {:.2}", best_ask - best_bid);
```

---

## 3. Account Endpoints (unsigned)

Account query endpoints are public — they take a `Pubkey` or string address and require no signature.

### `get_account(user)`

Returns the full account state: margin, open positions, open orders, and leverage settings.

```rust
use solana_pubkey::Pubkey;
use std::str::FromStr;

let pubkey = Pubkey::from_str("YOUR_PUBKEY_BASE58")?;
let account: AccountData = client.get_account(pubkey).await?;

println!("balance={:.2}", account.margin.total_balance);
for pos in &account.positions {
    println!("  {} size={} entry={}", pos.symbol, pos.signed_size, pos.entry_price);
}
```

### `get_open_orders(user)`

Returns all resting orders for an account.

```rust
let orders: Vec<OrderState> = client.get_open_orders("YOUR_PUBKEY_BASE58").await?;
for o in &orders {
    println!("oid={} {} {} px={} sz={}", o.order_id, o.symbol, o.side, o.price, o.size);
}
```

### Paginated account history

All six history methods use the public, unsigned `POST /api/v1/account` endpoint,
return exactly one typed `HistoryPage<T>`, and never follow `nextCursor`
automatically:

| Method | Request `type` | Row type |
|---|---|---|
| `get_fills_page` | `fills` | `HistoryFill` |
| `get_positions_page` | `positions` | `ClosedPosition` |
| `get_funding_page` | `fundingHistory` | `FundingPayment` |
| `get_orders_page` | `orderHistory` | `TerminalOrder` |
| `get_activity_page` | `activityHistory` | `AccountActivity` |
| `get_risk_page` | `riskHistory` | `RiskEvent` |

```rust
use bulk_client::msgs::{HistoryQuery, HistoryPage, HistoryFill};

let first: HistoryPage<HistoryFill> = client
    .get_fills_page(
        "YOUR_PUBKEY_BASE58",
        &HistoryQuery {
            limit: Some(100),
            start_slot: Some(1_000_000),
            ..HistoryQuery::default()
        },
    )
    .await?;

for fill in &first.data {
    println!("trade={} px={} amount={}", fill.trade_id, fill.price, fill.amount);
}

if let Some(cursor) = first.page.next_cursor {
    let next = client
        .get_fills_page(
            "YOUR_PUBKEY_BASE58",
            &HistoryQuery {
                limit: Some(100),
                cursor: Some(cursor),
                ..HistoryQuery::default()
            },
        )
        .await?;
    println!("loaded {} more fills", next.data.len());
}
```

`HistoryQuery` is flattened into the `/account` JSON body beside `user` and
`type`. It supports `limit`, `cursor`, `start_slot`, and `end_slot`; unset fields
are omitted. On a first page, `start_slot` and `end_slot` are inclusive and
`limit` is `1..=5000`, defaulting to `5000` when omitted. A continuation may
change `limit`, but must send only `limit` and the opaque `cursor`, without slot
bounds. Non-success responses return `HistoryHttpError::Api { status, body }`,
preserving the server error code and message.

`page.coverage` is the canonical completeness signal. `min_available_slot`
identifies the oldest retained slot when the server knows that boundary.

---

## 4. Trading Endpoints (signed)

All trading endpoints require a `TransactionSigner`. Every method accepts two optional
trailing parameters:

- **`account: Option<Pubkey>`** — override the target account (e.g. when acting as an agent
  wallet for a sub-account). Defaults to the signer's own public key.
- **`nonce: Option<u64>`** — supply a deterministic nonce for testing or cross-system
  coordination. Defaults to a monotonic timestamp-derived value.

Pass `None` for both in normal usage.

### `place_tx(actions, account, nonce)` — raw transaction

The primitive used by all convenience wrappers. Accepts any mix of `Action` variants,
builds a signed `Transaction`, and returns one `Response` per action.

```rust
use bulk_sdk::{Action, LimitOrder, CancelAll, ActionMeta, Side, TimeInForce};
use std::sync::Arc;

let actions = vec![
    Action::LimitOrder(LimitOrder {
        symbol: Arc::from("BTC-USD"),
        is_buy: true,
        price: 94_000.0,
        size: 0.05,
        tif: TimeInForce::GTC,
        reduce_only: false,
        iso: false,
        meta: ActionMeta::default(),
    }),
    Action::LimitOrder(LimitOrder {
        symbol: Arc::from("BTC-USD"),
        is_buy: true,
        price: 93_000.0,
        size: 0.05,
        tif: TimeInForce::GTC,
        reduce_only: false,
        iso: false,
        meta: ActionMeta::default(),
    }),
];

let responses = client.place_tx(actions, None, None).await?;
for resp in &responses {
    println!("oid={:?} status={}", resp.order_id, resp.status);
}
```

### `place_limit_order(symbol, side, price, size, tif, reduce_only, account, nonce)`

Places a single passive limit order.

| Argument | Type | Description |
|---|---|---|
| `symbol` | `&str` | Market symbol, e.g. `"BTC-USD"` |
| `side` | `Side` | `Side::Buy` or `Side::Sell` |
| `price` | `f64` | Limit price |
| `size` | `f64` | Order quantity |
| `tif` | `TimeInForce` | `GTC` \| `IOC` \| `ALO` (add-liquidity-only) |
| `reduce_only` | `bool` | If `true`, order can only reduce an existing position |
| `account` | `Option<Pubkey>` | Override account (see §4 intro) |
| `nonce` | `Option<u64>` | Override nonce (see §4 intro) |

```rust
use bulk_sdk::{Side, TimeInForce};

let resp = client
    .place_limit_order("BTC-USD", Side::Buy, 95_000.0, 0.1, TimeInForce::GTC, false, None, None)
    .await?;

println!("placed oid={:?}", resp.order_id);
```

### `place_market_order(symbol, side, size, reduce_only, account, nonce)`

Places an aggressive market order.

```rust
let resp = client
    .place_market_order("ETH-USD", Side::Sell, 1.0, false, None, None)
    .await?;
```

### `cancel_order(symbol, order_id, account, nonce)`

Cancels a single order by its ID.

```rust
let resp = client
    .cancel_order("BTC-USD", &existing_order_id, None, None)
    .await?;
```

### `cancel_all(symbols, account, nonce)`

Cancels all resting orders, optionally filtered to specific symbols. Pass an empty
`Vec` to cancel across every symbol.

```rust
// Cancel everything
let resp = client.cancel_all(vec![], None, None).await?;

// Cancel only BTC-USD and ETH-USD
let resp = client
    .cancel_all(vec!["BTC-USD".into(), "ETH-USD".into()], None, None)
    .await?;
```

---

## 5. Settings Endpoints (signed)

### `update_leverage(settings, account, nonce)`

Updates maximum leverage for one or more markets in a single transaction.

```rust
use std::collections::HashMap;

let mut settings = HashMap::new();
settings.insert("BTC-USD".into(), 20.0);
settings.insert("ETH-USD".into(), 10.0);

let resp = client.update_leverage(settings, None, None).await?;
```

### `manage_agent_wallet(agent_pubkey, delete, account, nonce)`

Authorises or revokes an agent wallet. An authorised agent can sign transactions on behalf
of the account without holding its private key.

| Argument | Type | Description |
|---|---|---|
| `agent_pubkey` | `Pubkey` | The agent's public key |
| `delete` | `bool` | `true` to revoke, `false` to authorise |
| `account` | `Option<Pubkey>` | Override account |
| `nonce` | `Option<u64>` | Override nonce |

```rust
use solana_pubkey::Pubkey;
use std::str::FromStr;

let agent = Pubkey::from_str("AGENT_PUBKEY_BASE58")?;

// Authorise
let resp = client.manage_agent_wallet(agent, false, None, None).await?;

// Revoke
let resp = client.manage_agent_wallet(agent, true, None, None).await?;
```

---

## 6. Testnet Endpoints (signed, testnet only)

### `request_faucet(user, amount, nonce)`

Requests testnet funds. All accounts can call this to receive a standard top-up;
whitelisted accounts may optionally specify an `amount`.

```rust
// Standard top-up for the signer's own account
let resp = client.request_faucet(None, None, None).await?;

// Whitelisted account requesting a specific amount
let resp = client.request_faucet(None, Some(10_000.0), None).await?;
```

### `whitelist_faucet(target_account, whitelist, account, nonce)`

Adds or removes a `target_account` from the faucet whitelist. **Testnet admin only.**

```rust
let target = Pubkey::from_str("TARGET_PUBKEY_BASE58")?;

// Add to whitelist
let resp = client.whitelist_faucet(target, true, None, None).await?;

// Remove from whitelist
let resp = client.whitelist_faucet(target, false, None, None).await?;
```

---

## 7. The `Response` Type

Every trading and settings method returns a `Response` (or `Vec<Response>` for `place_tx`).

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

## 8. Full Examples

### Read-only: fetch market snapshot

```rust
use bulk_sdk::BulkHttpClient;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkHttpClient::with_url("https://exchange.bulk.trade", None, None)?;

    let ticker = client.get_ticker("BTC-USD").await?;
    println!("BTC mark={} funding={:.6}", ticker.mark_price, ticker.funding_rate);

    let book = client.get_orderbook("BTC-USD", Some(5), None).await?;
    let (bids, asks) = &book.levels;
    println!("best bid={} best ask={}",
        bids[0].price, asks[0].price);

    Ok(())
}
```

### Authenticated: place a ladder and cancel all on error

```rust
use bulk_sdk::{
    Action, ActionMeta, BulkHttpClient, LimitOrder, Side, SignatureDomain, TimeInForce,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkHttpClient::with_url(
        "https://exchange.bulk.trade",
        Some("YOUR_BASE58_PRIVATE_KEY"),
        Some(SignatureDomain::Mainnet),
    )?;

    // Ladder: three bids at descending price levels
    let actions: Vec<Action> = [95_000.0, 94_000.0, 93_000.0]
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
                meta: ActionMeta::default(),
            })
        })
        .collect();

    let responses = client.place_tx(actions, None, None).await?;
    let any_error = responses.iter().any(|r| r.is_error());

    if any_error {
        eprintln!("one or more placements failed — cancelling all");
        for r in &responses {
            if r.is_error() {
                eprintln!("  {:?}", r.message);
            }
        }
        client.cancel_all(vec!["BTC-USD".into()], None, None).await?;
    } else {
        for r in &responses {
            println!("placed oid={:?} status={}", r.order_id, r.status);
        }
    }

    Ok(())
}
```
