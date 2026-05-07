<p align="center">
  <img src="bulkclient.png" alt="Bulk Client" width="400" />
</p>

<p align="center">
  <strong>High-performance client SDK for BULK</strong>
</p>

<p align="center">
  <a href="https://crates.io/crates/bulk-client"><img src="https://img.shields.io/crates/v/bulk-client.svg" alt="crates.io" /></a>
  <a href="https://docs.rs/bulk-client"><img src="https://docs.rs/bulk-client/badge.svg" alt="docs.rs" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License" /></a>
</p>

---

HTTP and WebSocket clients for the [Bulk exchange](https://bulk.trade), written in **Rust** and **Python**.

## Features

- **WebSocket** — Actor + Watch architecture for zero-cost ticker reads and low-latency order placement
- **HTTP** — Full REST API coverage (market data, account queries, signed trading)
- **Batch transactions** — Bundle multiple actions (orders, cancels, conditionals) into a single signed transaction
- **Conditional orders** — Stop, take-profit, OCO/range, trailing stop, trigger baskets, on-fill consequents
- **Sub-accounts & multisig** — First-class support for sub-account management and multisig smart accounts
- **Ed25519 signing** — Native signing with wincode binary serialization

## Quickstart (Rust)

```rust
use bulk_client::*;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = BulkWsClient::connect(WSConfig {
        url: "wss://exchange-wss.bulk.trade".into(),
        symbols: vec!["BTC-USD".into()],
        ..Default::default()
    }).await?;

    if let Some(ticker) = client.get_ticker("BTC-USD") {
        println!("BTC mark: {}", ticker.mark_price);
    }

    client.shutdown().await;
    Ok(())
}
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
