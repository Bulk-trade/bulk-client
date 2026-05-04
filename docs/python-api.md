# `BulkWebSocketClient` — Python API Reference

## Architecture Overview

`BulkWebSocketClient` is an `asyncio`-based WebSocket client. All state
(tickers, order books, account snapshot, open orders) is maintained directly on
the client object and updated in-place as messages arrive from the exchange.
A single background `asyncio.Task` runs the receive loop; the public API is a
set of `async` methods and synchronous accessors on the same object.

```
  ┌──────────────────────┐   websockets   ┌──────────────────┐
  │ BulkWebSocketClient  │ ◀────────────▶ │  Exchange WSS    │
  │                      │                └──────────────────┘
  │  self.tickers        │  ← ticker / account updates applied in-place
  │  self.open_orders    │
  │  self.order_books    │  ← OrderBook instances updated from l2 streams
  │  self.margin         │
  │  self.inventory      │  ← Inventory tracks positions + P&L
  └──────────────────────┘
         │
         │  asyncio.Future  (per request_id)
         ▼
  place_orders() / place_limit_order() / ...
  await response
```

Events are dispatched via registered callbacks (`on(topic, handler)`). Both
`async def` and plain `def` handlers are supported — the client detects which
and calls accordingly.

---

## 1. Instantiating `BulkWebSocketClient`

The constructor is synchronous. The actual WebSocket connection is established
by calling `await client.connect()`.

### Constructor parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `url` | `str` | `"wss://exchange-wss.bulk.trade"` | WebSocket endpoint |
| `symbols` | `List[str]` | `["BTC-USD", "ETH-USD", "SOL-USD"]` | Symbols to auto-subscribe tickers for |
| `signer` | `Optional[TransactionSigner]` | `None` | Required for any trading operation |
| `inventory` | `Optional[Inventory]` | `Inventory()` | Position and P&L tracker; a fresh one is created if omitted |
| `logger` | `Optional[logging.Logger]` | module logger | Pass your own logger to integrate with your log pipeline |
| `handlers` | `Optional[Dict[Topic, Callable]]` | `None` | Register event handlers at construction time |
| `debug` | `bool` | `False` | Wraps the socket in `LoggingWebSocket` for full message tracing |

### Without a signer — market data only

No private key is needed for read-only market data. Calling any trading method
on a signer-less client raises `RuntimeError` immediately.

```python
import asyncio
from bulk_api import BulkWebSocketClient

async def main():
    client = BulkWebSocketClient(
        url="wss://exchange-wss.bulk.trade",
        symbols=["BTC-USD", "ETH-USD"],
        # signer omitted — read-only mode
    )

    connected = await client.connect()
    if not connected:
        raise RuntimeError("Failed to connect")

    # Tickers for `symbols` are auto-subscribed on connect.
    await asyncio.sleep(1)  # wait for first ticker message
    ticker = client.get_ticker("BTC-USD")
    if ticker:
        print(f"BTC mark price: {ticker.mark_price}")

    await client.disconnect()

asyncio.run(main())
```

### With a signer — trading enabled

```python
import asyncio
from bulk_api import BulkWebSocketClient
from bulk_api.common.signer import TransactionSigner

async def main():
    signer = TransactionSigner("YOUR_BASE58_PRIVATE_KEY")

    client = BulkWebSocketClient(
        url="wss://exchange-wss.bulk.trade",
        symbols=["BTC-USD", "SOL-USD"],
        signer=signer,
    )

    await client.connect()
    print(f"Connected. Balance: {client.margin.total_balance:.2f}")

    await client.disconnect()

asyncio.run(main())
```

When a `signer` is provided, `connect()` automatically subscribes to:
- The **account stream** (margin, positions, open orders, leverage settings).
- **Ticker streams** for every symbol in `symbols`.

### Registering handlers at construction time

You can pass an initial handler map to the constructor to avoid a window between
`connect()` and your first `on()` call:

```python
from bulk_api.common import Topic

async def on_ticker(ticker):
    print(f"[ticker] {ticker.symbol} mark={ticker.mark_price}")

client = BulkWebSocketClient(
    symbols=["BTC-USD"],
    signer=signer,
    handlers={
        Topic.TICKER: on_ticker,
    },
)
await client.connect()
```

### Generating a new keypair (testing only)

```python
signer = TransactionSigner.generate_account()
print(f"public key: {signer.public_key}")
# signer.private_key holds the base58-encoded seed — store it securely
```

### Connection lifecycle

```python
# Check connection state synchronously
if client.is_connected:
    ...

# Graceful shutdown
await client.disconnect()
```

#### Automatic reconnection

The client reconnects automatically on network errors using exponential
back-off, starting at 1 second and capping at 30 seconds. All active
subscriptions are replayed once the connection is re-established — no manual
re-subscription is required.

---

## 2. Transactions — Actions and Batching

Every trading operation is expressed as a list of **action** objects passed to
`place_orders()`. The client assembles them into a single signed transaction,
sends it over the WebSocket, and awaits one `OrderResponse` per action.

### Action types

| Class | Description |
|---|---|
| `LimitOrder` | Passive limit order |
| `MarketOrder` | Aggressive market order |
| `CancelOrder` | Cancel a specific order by ID |
| `CancelAll` | Cancel all orders, optionally filtered by symbol(s) |
| `OraclePrice` | Oracle price update (separate `update_oracle()` path) |

All classes live in `bulk_api.messages.trade`.

### Single-action transactions

The convenience wrappers (`place_limit_order`, `place_market_order`,
`cancel_order`, `cancel_all`) each wrap a single action and return the single
`OrderResponse` directly:

```python
resp = await client.place_limit_order("BTC-USD", Side.BUY, 95_000.0, 0.1)
```

### Multi-action transactions (batching)

`place_orders(actions)` lets you bundle multiple actions into a single signed
transaction. Actions are processed atomically by the exchange in the order
given, and you receive one `OrderResponse` per action back.

This is useful for:

- **Ladder entry**: placing several limit orders at different price levels in
  one round-trip.
- **Atomic replace**: cancelling an existing order and placing a replacement in
  the same transaction, with no window where neither order is resting.

```python
from bulk_api.messages.trade import LimitOrder, CancelOrder
from bulk_api.common import Side, TimeInForce

# --- Ladder: three limit buys at different price levels ---
actions = [
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=p, size=0.05)
    for p in [94_000.0, 93_000.0, 92_000.0]
]

responses = await client.place_orders(actions)
for resp in responses:
    print(f"order_id={resp.order_id} status={resp.status}")
```

```python
# --- Atomic replace: cancel + re-place in one tx ---
cancel = CancelOrder(symbol="BTC-USD", oid=existing_order_id, side=Side.BUY)
new_order = LimitOrder(symbol="BTC-USD", side=Side.BUY, price=94_500.0, size=0.1)

# Both actions share the same nonce and signature.
responses = await client.place_orders([cancel, new_order])
```

### Optional `nonce` and `timeout` parameters

All trading methods accept:

- **`nonce`**: supply a deterministic nonce (useful in tests). Defaults to
  `time.time_ns()`.
- **`timeout`**: per-request response deadline in seconds. Defaults to
  `client.default_timeout` (10 s). Raises `asyncio.TimeoutError` on expiry.

---

## 3. Executing Transactions — Examples

### Convenience wrappers

#### Limit order

```python
from bulk_api.common import Side, TimeInForce

resp = await client.place_limit_order(
    symbol="BTC-USD",
    side=Side.BUY,
    price=95_000.0,
    size=0.1,
    reduce_only=False,
    time_in_force=TimeInForce.GTC,  # GTC | IOC | ALO
    timeout=5.0,                     # optional, overrides default_timeout
)
print(f"placed order_id={resp.order_id}")
```

#### Market order

```python
resp = await client.place_market_order(
    symbol="ETH-USD",
    side=Side.SELL,
    size=1.0,
    reduce_only=False,
)
```

#### Cancel a specific order

```python
resp = await client.cancel_order(
    side=Side.BUY,
    symbol="BTC-USD",
    order_id=existing_order_id,
)
```

#### Cancel all orders (optionally filtered)

```python
# Cancel everything across all symbols.
resp = await client.cancel_all()

# Cancel only BTC-USD and ETH-USD orders.
resp = await client.cancel_all(symbols=["BTC-USD", "ETH-USD"])
```

### Reading account state

All reads below are synchronous — no `await` needed.

```python
# Margin / collateral
if client.margin:
    print(f"balance={client.margin.total_balance:.2f}  "
          f"available={client.margin.available_balance:.2f}")

# Open orders — optionally filtered by symbol
all_orders  = client.get_orders()
btc_orders  = client.get_orders(symbol="BTC-USD")
order_map   = client.get_order_map()   # Dict[order_id, OrderState]

# P&L via Inventory
pnl = client.get_pnl()
print(f"realized={pnl.realized:.2f}  unrealized={pnl.unrealized:.2f}  net={pnl.net:.2f}")

# Current ticker
ticker = client.get_ticker("BTC-USD")
if ticker:
    print(f"BTC mark price: {ticker.mark_price}")

# Order book (after subscribing)
book = client.get_book("BTC-USD")
if book:
    print(f"best bid={book.best_bid()}  best ask={book.best_ask()}")

# Leverage settings
lev = client.leverage_settings.get("BTC-USD")
if lev:
    print(f"BTC leverage: {lev.leverage}x")
```

---

## 4. Subscribing to Market Data

### Auto-subscriptions on connect

When `connect()` is called with a `signer` present, the client automatically
subscribes to:
- **Account stream** (`margin`, `positions`, `open_orders`, `leverage_settings`).
- **Ticker streams** for every symbol in `symbols`.

For a read-only (no signer) client, only the ticker streams are auto-subscribed.

### Subscribing dynamically after connect

#### Ticker

```python
await client.subscribe_ticker("SOL-USD")

ticker = client.get_ticker("SOL-USD")
if ticker:
    print(f"SOL mark price: {ticker.mark_price}")
```

#### L2 order book

```python
# Full snapshot first — creates and populates an OrderBook instance.
await client.subscribe_orderbook_snapshot("BTC-USD", nlevels=20)

# Then subscribe to incremental deltas to keep it current.
await client.subscribe_orderbook_delta("BTC-USD")

book = client.get_book("BTC-USD")
```

#### Trades and candles

```python
# Public trade feed for one or more symbols.
await client.subscribe_trades(["BTC-USD", "ETH-USD"])

# OHLCV candles — interval strings: "1min", "5min", "15min", "1h", "4h", "1d"
await client.subscribe_candles("BTC-USD", "5min")
```

#### Account stream (manual)

```python
# Subscribe to account updates for a specific public key.
# (Called automatically when signer is set.)
await client.subscribe_account(signer.public_key)
```

### Event callbacks with `on()` and `off()`

Register a callback for any `Topic`. Both plain functions and `async def`
coroutines are accepted — the client detects which and awaits accordingly.

```python
from bulk_api.common import Topic

# Plain sync callback
def on_ticker(ticker):
    print(f"[ticker] {ticker.symbol} mark={ticker.mark_price}")

# Async callback
async def on_fill(fill):
    print(f"[fill] {fill.symbol} {fill.side} {fill.size} @ {fill.price} maker={fill.is_maker}")

client.on(Topic.TICKER,   on_ticker)
client.on(Topic.FILL,     on_fill)
client.on(Topic.ORDER,    lambda state: print(f"[order] {state.order_id} {state.status}"))
client.on(Topic.MARGIN,   lambda m: print(f"[margin] balance={m.total_balance:.2f}"))
client.on(Topic.POSITION, lambda pos: print(f"[position] {pos.symbol} size={pos.size}"))
client.on(Topic.LEVERAGE, lambda settings: print(f"[leverage] {len(settings)} symbols updated"))
client.on(Topic.ACCOUNT,  lambda snap: print(f"[account] {len(snap.positions)} positions"))
client.on(Topic.ERROR,    lambda err: print(f"[error] {err}"))

# Remove a handler
client.off(Topic.TICKER, on_ticker)
```

#### Topic reference

| `Topic` | Callback receives | Fired when |
|---|---|---|
| `Topic.TICKER` | `Ticker` | Mark price or funding rate changes |
| `Topic.TRADES` | `List[Trade]` | A public trade executes |
| `Topic.L2SNAPSHOT` | `L2Snapshot` | Full order book snapshot arrives |
| `Topic.L2DELTA` | `L2Delta` | Incremental book update arrives |
| `Topic.CANDLE` | `Candle` | A candle bar closes or updates |
| `Topic.ACCOUNT` | `AccountSnapshot` | Initial account state on connect |
| `Topic.MARGIN` | `MarginUpdate` | Margin / collateral changes |
| `Topic.POSITION` | `PositionUpdate` | A position changes size or P&L |
| `Topic.ORDER` | `OrderState` | Order created, amended, or reaches terminal state |
| `Topic.FILL` | `Fill` | An order is partially or fully filled |
| `Topic.LEVERAGE` | `List[LeverageSetting]` | Leverage settings change |
| `Topic.ERROR` | `dict` | Exchange returns an error response |

### Full example — market-making skeleton

```python
import asyncio
import logging
from bulk_api import BulkWebSocketClient
from bulk_api.common import Side, TimeInForce, Topic
from bulk_api.common.signer import TransactionSigner

logging.basicConfig(level=logging.INFO)

async def main():
    signer = TransactionSigner("YOUR_BASE58_PRIVATE_KEY")

    client = BulkWebSocketClient(
        url="wss://exchange-wss.bulk.trade",
        symbols=["BTC-USD"],
        signer=signer,
    )

    # Subscribe to book data
    await client.connect()
    await client.subscribe_orderbook_snapshot("BTC-USD", nlevels=5)
    await client.subscribe_orderbook_delta("BTC-USD")

    # React to every ticker — place a bid 10 bps below mid
    async def on_ticker(ticker):
        if ticker.symbol != "BTC-USD":
            return
        bid = ticker.mark_price * 0.999
        try:
            resp = await client.place_limit_order(
                "BTC-USD", Side.BUY, bid, 0.01,
                time_in_force=TimeInForce.ALO,
            )
            print(f"Resting bid @ {bid:.1f} → order_id={resp.order_id}")
        except Exception as e:
            print(f"Order error: {e}")

    async def on_fill(fill):
        pnl = client.get_pnl()
        print(f"Fill {fill.size} @ {fill.price} | net PnL={pnl.net:.2f}")

    client.on(Topic.TICKER, on_ticker)
    client.on(Topic.FILL, on_fill)

    # Keep running until interrupted
    try:
        await asyncio.sleep(float("inf"))
    except asyncio.CancelledError:
        pass
    finally:
        await client.cancel_all()
        await client.disconnect()

asyncio.run(main())
```

---
