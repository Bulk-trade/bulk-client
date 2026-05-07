<p align="center">
  <img src="bulkclient.png" alt="Bulk Client" width="400" />
</p>

<p align="center">
  <strong>High-performance client SDK for BULK</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/bulk-client"><img src="https://img.shields.io/crates/v/bulk-client.svg" alt="crates.io" /></a>
  <a href="https://crates.io/crates/bulk-cli"><img src="https://img.shields.io/crates/v/bulk-cli.svg?label=bulk-cli" alt="bulk-cli" /></a>
  <a href="https://pypi.org/project/bulk-client"><img src="https://img.shields.io/pypi/v/bulk-client.svg" alt="PyPI" /></a>
  <a href="https://docs.rs/bulk-client"><img src="https://docs.rs/bulk-client/badge.svg" alt="docs.rs" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License" /></a>
</p>

---

HTTP and WebSocket clients for [BULK](https://bulk.trade), written in **Rust** and **Python**.

## Install

```bash
# Rust SDK
cargo add bulk-client

# CLI
cargo install bulk-cli

# Python
pip install bulk-client
```

## Features

- **WebSocket** - Actor + Watch architecture for zero-cost ticker reads and low-latency order placement
- **HTTP** - Full REST API coverage (market data, account queries, signed trading)
- **Batch transactions** - Bundle multiple actions (orders, cancels, conditionals) into a single signed transaction
- **Conditional orders** - Stop, take-profit, OCO/range, trailing stop, trigger baskets, on-fill consequents
- **Sub-accounts & multisig** - First-class support for sub-account management and multisig smart accounts
- **Ed25519 signing** - Native signing with wincode binary serialization

## Architecture

BULK client is built for latency-sensitive trading. Here's what makes it fast:

### Actor + Watch (WebSocket)

The WebSocket client uses an **actor + watch** pattern. A single background actor owns the
socket and deserializes the stream into shared state. Consumers read via `tokio::watch`
channels - **zero-copy, no lock contention, readers never block the writer**. Getting the
latest ticker is a `.borrow()`, not an async round-trip.

```text
 ┌──────────────┐         mpsc::channel           ┌───────────────┐
 │ BulkWsClient │ ───── Command ────────────────▶ │     Actor     │
 │   (handle)   │ ◀──── watch::Receiver ────────  │  (owns state) │
 └──────────────┘                                 └───────┬───────┘
       │                                                  │
       │ oneshot for order responses                      │ tokio::select!
       └─────────────────────────────────────────────────▶│◀── ws_read
```

### Binary Signing (Wincode)

Transactions are serialized to a **compact little-endian binary** format, not JSON.
Prices and sizes are **fixed-point `u64`** (1e8 scale) - no floating-point ambiguity,
significantly faster than text serialization. Ed25519 signatures are computed over this
canonical binary representation.

### Batch Transactions

Multiple actions go into a **single signed transaction** with one signature and one
network round-trip. 

## Quickstart (Rust)

### Connect and Read Market Data

```rust
use bulk_client::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkWsClient::connect(WSConfig {
        url: "wss://exchange-wss.bulk.trade".into(),
        symbols: vec!["BTC-USD".into()],
        ..Default::default()
    }).await?;

    // Zero-cost read - no lock, no async round-trip
    if let Some(ticker) = client.get_ticker("BTC-USD") {
        println!("BTC mark: {}", ticker.mark_price);
    }

    client.shutdown().await;
    Ok(())
}
```

### Place a Limit Order (HTTP)

```rust
use bulk_client::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkHttpClient::with_url(
        "https://exchange-api.bulk.trade/api/v1",
        Some("your_base58_private_key"),
    )?;

    let resp = client.place_limit_order(
        "BTC-USD", Side::Buy, 95_000.0, 0.01,
        TimeInForce::GTC, false, None, None,
    ).await?;

    println!("order status: {}", resp.status);
    Ok(())
}
```

### Batch: Place + Cancel in One Transaction

```rust
use std::sync::Arc;
use bulk_client::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkHttpClient::with_url(
        "https://exchange-api.bulk.trade/api/v1",
        Some("your_base58_private_key"),
    )?;

    let actions = vec![
        Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 94_000.0,
            size: 0.01,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            meta: Default::default(),
        }),
        Action::CancelAll(CancelAll {
            symbols: vec!["ETH-USD".into()],
            meta: Default::default(),
        }),
    ];

    let responses = client.place_tx(actions, None, None).await?;
    for r in &responses {
        println!("{}: {}", r.status, r.order_id.as_deref().unwrap_or("-"));
    }
    Ok(())
}
```

### Conditional: OCO Range (Stop + Take-Profit)

```rust
use std::sync::Arc;
use bulk_client::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkHttpClient::with_url(
        "https://exchange-api.bulk.trade/api/v1",
        Some("your_base58_private_key"),
    )?;

    // Single transaction: collar stop at 90k, TP at 110k
    let actions = vec![
        Action::Range(Range {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            size: 0.1,
            collar_min: 90_000.0,
            collar_max: 110_000.0,
            limit_min: Some(89_900.0),
            limit_max: Some(110_100.0),
            meta: Default::default(),
        }),
    ];

    let responses = client.place_tx(actions, None, None).await?;
    println!("OCO placed: {}", responses[0].status);
    Ok(())
}
```

## Quickstart (Python)

### Read Market Data

```python
from bulk_client import BulkHttpClient

client = BulkHttpClient(base_url="https://exchange-api.bulk.trade/api/v1")

ticker = client.get_ticker("BTC-USD")
print(f"BTC last: {ticker['lastPrice']}")

book = client.get_orderbook("BTC-USD", nlevels=5)
print(f"Best bid: {book['levels'][0][0]}")
```

### Place a Limit Order

```python
from bulk_client import BulkHttpClient
from bulk_api.common import Side, TimeInForce
from bulk_api.messages import LimitOrder

client = BulkHttpClient(
    base_url="https://exchange-api.bulk.trade/api/v1",
    private_key="your_base58_private_key",
)

resp = client.place_orders([
    LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=95_000.0,
        size=0.01,
        time_in_force=TimeInForce.GTC,
    )
])

print(f"Order result: {resp}")
```

### Cancel All Orders

```python
from bulk_api.messages import CancelAll

resp = client.place_orders([
    CancelAll(symbols=["BTC-USD"])
])
```

## CLI

The `bulk` CLI wraps the Rust SDK for quick terminal trading:

```bash
# Place a limit order
bulk place Buy BTC-USD 0.01@95000 --tif GTC

# Place a market order
bulk place Sell ETH-USD 1.0

# Cancel a specific order
bulk cancel BTC-USD <order-id>

# Cancel all orders on a market
bulk cancel-all --instrument BTC-USD

# Conditional orders
bulk stop BTC-USD 0.1 90000                     # stop-loss
bulk tp BTC-USD 0.1 110000 --above              # take-profit
bulk range BTC-USD 0.1 90000 110000 --buy       # OCO collar
bulk trail BTC-USD 0.1 --buy --trail-bps 200    # trailing stop

# Sub-accounts
bulk create-subaccount mybot --margin-symbol USDC --margin-amount 1000
bulk transfer <from> <to> USDC 500

# Multisig
bulk create-multisig <pk1>,<pk2> --threshold 2 --lock 120
```

Set your key via environment variable or flag:

```bash
export BULK_PRIVATE_KEY="your_base58_private_key"
export BULK_API_URL="https://exchange-api.bulk.trade/api/v1"
```

## Documentation

- Rust API
  - [WebSocket](docs/rust-ws-api.md)
  - [HTTP](docs/rust-http-api.md)
- Python API
  - [WebSocket](docs/python-ws-api.md)
  - [HTTP](docs/python-http-api.md)

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
