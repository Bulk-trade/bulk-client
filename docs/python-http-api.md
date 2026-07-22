# `BulkHttpClient` — Python HTTP REST API Reference

`BulkHttpClient` provides synchronous request/response access to the Bulk exchange over HTTP.
It covers three categories of endpoints:

- **Market data** — public, unsigned (tickers, order books, candles)
- **Account queries** — public, unsigned (account state, open orders, paginated history)
- **Trading & settings** — private, signed (orders, cancels, leverage, agent wallets)

---

## 1. Instantiating `BulkHttpClient`

```python
BulkHttpClient(
    base_url: str = "https://exchange-api2.bulk.trade/api/v1",
    private_key: Optional[str] = None,
    signature_domain: Optional[SignatureDomain] = None,
    timeout: int = 10
)
```

| Parameter | Type | Default | Description |
|---|---|---|---|
| `base_url` | `str` | `"https://exchange-api2.bulk.trade/api/v1"` | HTTP endpoint |
| `private_key` | `Optional[str]` | `None` | Base58-encoded private key. Required for trading and settings operations |
| `signature_domain` | `Optional[SignatureDomain]` | `None` | Required when `private_key` is set; selects mainnet, testnet, or devnet |
| `timeout` | `int` | `10` | Per-request timeout in seconds |

### Read-only client (market data + account queries)

No private key is needed for public endpoints. Attempts to call any trading or
settings method on an unauthenticated client will raise a `ValueError` immediately.

```python
from bulk_api import BulkHttpClient

client = BulkHttpClient()
```

### Authenticated client (trading enabled)

```python
import os
from bulk_api import BulkHttpClient
from bulk_api.common import SignatureDomain

client = BulkHttpClient(
    private_key=os.environ["BULK_PRIVATE_KEY"],
    signature_domain=SignatureDomain.MAINNET,
)
print(f"Trading as: {client.signer.public_key}")
```

---

## 2. Market Data Endpoints (unsigned)

### `get_exchange_info()`

Returns metadata for all available markets.

```python
info = client.get_exchange_info()
for symbol, market in info["markets"].items():
    print(f"{symbol}: tick={market['tickSize']} lot={market['lotSize']}")
```

**Returns** `dict` with keys:

| Key | Description |
|---|---|
| `symbols` | List of available market symbols |
| `markets` | Dict of symbol → market detail (tick size, lot size, etc.) |

### `get_ticker(symbol)`

Returns the current ticker and statistics for a single market.

```python
ticker = client.get_ticker("BTC-USD")
print(f"mark={ticker['markPrice']} funding={ticker['fundingRate']:.6f}")
```

**Returns** `dict` with keys:

| Key | Description |
|---|---|
| `symbol` | Market symbol |
| `lastPrice` | Last traded price |
| `markPrice` | Current mark price |
| `oraclePrice` | Current oracle price |
| `fundingRate` | Current funding rate |
| `openInterest` | Total open interest |
| `volume24h` | 24-hour trading volume |
| `highPrice24h` / `lowPrice24h` | 24-hour high / low |
| `priceChange24h` / `priceChangePercent24h` | 24-hour price change |

### `get_klines(symbol, interval, start_time, end_time, limit)`

Returns historical OHLCV candles.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `symbol` | `str` | — | Market symbol, e.g. `"BTC-USD"` |
| `interval` | `Literal[...]` | — | `"1m"` `"5m"` `"15m"` `"30m"` `"1h"` `"4h"` `"1d"` `"1w"` |
| `start_time` | `Optional[int]` | `None` | Start timestamp in milliseconds |
| `end_time` | `Optional[int]` | `None` | End timestamp in milliseconds |
| `limit` | `int` | `500` | Max candles to return (max `1000`) |

```python
# Last 100 hourly candles
candles = client.get_klines("BTC-USD", "1h", limit=100)
for c in candles:
    print(f"t={c['t']} o={c['o']} h={c['h']} l={c['l']} c={c['c']} v={c['v']}")
```

Each candle is a `dict` with compact keys: `t` (open time ms), `T` (close time ms),
`o`, `h`, `l`, `c` (OHLC prices), `v` (volume), `n` (trade count).

### `get_orderbook(symbol, nlevels, aggregation)`

Returns an L2 order book snapshot.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `symbol` | `str` | — | Market symbol |
| `nlevels` | `int` | `20` | Price levels per side (max `1000`) |
| `aggregation` | `Optional[float]` | `None` | Optional price grouping/bucketing |

```python
book = client.get_orderbook("BTC-USD", nlevels=5)
bids, asks = book["levels"]
print(f"best bid={bids[0][0]} best ask={asks[0][0]} spread={asks[0][0] - bids[0][0]:.2f}")
```

Each level is a `[price, size, num_orders]` list.

---

## 3. Account Endpoints (unsigned)

Account queries are public — they take a base58 public key string and require no signature.

### `get_full_account(user)`

Returns the complete account state: margin, open positions, open orders, and leverage settings.

```python
account = client.get_full_account("YOUR_PUBKEY_BASE58")
full = account["fullAccount"]
print(f"positions: {len(full['positions'])}")
print(f"open orders: {len(full['openOrders'])}")
```

### `get_open_orders(user)`

Returns all resting orders for an account.

```python
orders = client.get_open_orders("YOUR_PUBKEY_BASE58")
for entry in orders:
    o = entry["openOrder"]
    print(f"oid={o['orderId']} {o['coin']} px={o['price']} sz={o['size']}")
```

### Paginated account history

All six public history methods use the unsigned `POST /api/v1/account`
endpoint, return one typed `HistoryPage`, and never follow `next_cursor`
automatically:

| Method | Request `type` | Row type |
|---|---|---|
| `get_fills_page` | `fills` | `HistoryFill` |
| `get_positions_page` | `positions` | `ClosedPosition` |
| `get_funding_page` | `fundingHistory` | `FundingPayment` |
| `get_orders_page` | `orderHistory` | `TerminalOrder` |
| `get_activity_page` | `activityHistory` | `AccountActivity` |
| `get_risk_page` | `riskHistory` | `RiskEvent` |

```python
page = client.get_fills_page(
    "YOUR_PUBKEY_BASE58",
    limit=100,
    start_slot=1_000_000,
)
for fill in page.data:
    print(fill.symbol, fill.amount, fill.price, fill.slot, fill.sequence)

if page.page.next_cursor is not None:
    next_page = client.get_fills_page(
        "YOUR_PUBKEY_BASE58",
        limit=100,
        cursor=page.page.next_cursor,
    )
```

The keyword-only arguments are added to the `/account` JSON body beside `user`
and `type`; unset fields are omitted. On a first page, `start_slot` and
`end_slot` are inclusive and `limit` is `1..=5000`, defaulting to `5000` when
omitted. A continuation may change `limit`, but must send only `limit` and the
opaque `cursor`, without slot bounds. Non-success responses raise
`HistoryHttpError`, preserving `status` and the structured server body.

`HistoryPage.page.coverage` is the canonical completeness signal.
`min_available_slot` identifies the oldest retained slot when the server knows
that boundary.

---

## 4. Trading Endpoints (signed)

All trading endpoints require a `private_key` to have been passed at construction.
A `ValueError` is raised immediately if the client has no signer.

### Action types

`place_orders` accepts a list of action objects. Import them from `bulk_api.messages`:

```python
from bulk_api.messages import LimitOrder, MarketOrder, CancelOrder, CancelAll
from bulk_api.common import Side, TimeInForce
```

| Class | Description |
|---|---|
| `LimitOrder` | Passive limit order |
| `MarketOrder` | Aggressive market order |
| `CancelOrder` | Cancel a specific order by ID |
| `CancelAll` | Cancel all orders, optionally filtered to specific symbols |

### `place_orders(txns, nonce)`

Submits a list of actions as a single signed transaction. The exchange processes
them atomically in order.

| Parameter | Type | Description |
|---|---|---|
| `txns` | `List[LimitOrder \| MarketOrder \| CancelOrder \| CancelAll]` | Actions to include |
| `nonce` | `Optional[int]` | Override nonce (defaults to `time.time_ns()`) |

**Returns** the raw exchange JSON response. Use `OrderResponse.from_api()` to parse
individual action results (see §6).

#### Limit order

```python
from bulk_api.messages import LimitOrder
from bulk_api.common import Side, TimeInForce

resp = client.place_orders([
    LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=95_000.0,
        size=0.1,
        time_in_force=TimeInForce.GTC,
        reduce_only=False,
    )
])
```

`LimitOrder` fields:

| Field | Type | Default | Description |
|---|---|---|---|
| `symbol` | `str` | — | Market symbol |
| `side` | `Side` | — | `Side.BUY` or `Side.SELL` |
| `price` | `float` | — | Limit price |
| `size` | `float` | — | Order quantity |
| `time_in_force` | `TimeInForce` | `TimeInForce.GTC` | `GTC` \| `IOC` \| `ALO` |
| `reduce_only` | `bool` | `False` | If `True`, only reduces an existing position |

#### Market order

```python
from bulk_api.messages import MarketOrder

resp = client.place_orders([
    MarketOrder(symbol="ETH-USD", side=Side.SELL, size=1.0)
])
```

`MarketOrder` fields:

| Field | Type | Default | Description |
|---|---|---|---|
| `symbol` | `str` | — | Market symbol |
| `side` | `Side` | — | `Side.BUY` or `Side.SELL` |
| `size` | `float` | — | Order quantity |
| `reduce_only` | `bool` | `False` | If `True`, only reduces an existing position |

#### Cancel a specific order

```python
from bulk_api.messages import CancelOrder

resp = client.place_orders([
    CancelOrder(symbol="BTC-USD", oid="EXISTING_ORDER_ID_BASE58")
])
```

#### Cancel all orders (optionally filtered)

```python
from bulk_api.messages import CancelAll

# Cancel all open orders across every symbol
resp = client.place_orders([CancelAll(symbols=[])])

# Cancel only BTC-USD and ETH-USD
resp = client.place_orders([CancelAll(symbols=["BTC-USD", "ETH-USD"])])
```

#### Batching multiple actions

All actions in a single `place_orders` call are bundled into one signed transaction:

```python
# Ladder: three bids at descending price levels
resp = client.place_orders([
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=95_000.0, size=0.05),
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=94_000.0, size=0.05),
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=93_000.0, size=0.05),
])
```

```python
# Atomic replace: cancel + re-place in one transaction
resp = client.place_orders([
    CancelOrder(symbol="BTC-USD", oid=existing_order_id),
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=94_500.0, size=0.1),
])
```

---

## 5. Settings Endpoints (signed)

### `update_leverage(leverage_settings)`

Updates maximum leverage for one or more markets in a single transaction.

```python
client.update_leverage([
    ("BTC-USD", 20.0),
    ("ETH-USD", 10.0),
])
```

`leverage_settings` is a list of `(symbol, max_leverage)` tuples.

### `manage_agent_wallet(agent_pubkey, delete)`

Authorises or revokes an agent wallet. An authorised agent can sign transactions on
behalf of the account without holding its private key.

| Parameter | Type | Default | Description |
|---|---|---|---|
| `agent_pubkey` | `str` | — | Agent's base58 public key |
| `delete` | `bool` | `False` | `True` to revoke, `False` to authorise |

```python
# Authorise an agent
client.manage_agent_wallet("AGENT_PUBKEY_BASE58", delete=False)

# Revoke an agent
client.manage_agent_wallet("AGENT_PUBKEY_BASE58", delete=True)
```

---

## 6. Testnet Endpoints (signed, testnet only)

### `request_faucet(user, amount, nonce)`

Requests testnet funds. All accounts receive a standard top-up; whitelisted accounts
may specify a custom `amount`.

```python
# Standard top-up for the signer's own account
client.request_faucet()

# Specific amount (whitelisted accounts only)
client.request_faucet(amount=10_000.0)

# Top-up for another account
client.request_faucet(user="OTHER_PUBKEY_BASE58")
```

### `whitelist_faucet(target_account, whitelist, nonce)`

Adds or removes an account from the faucet whitelist. **Testnet admin only.**

```python
# Add to whitelist
client.whitelist_faucet("TARGET_PUBKEY_BASE58", whitelist=True)

# Remove from whitelist
client.whitelist_faucet("TARGET_PUBKEY_BASE58", whitelist=False)
```

---

## 7. The `OrderResponse` Type

Parse raw exchange responses from `place_orders` using `OrderResponse.from_api()`,
which returns a list — one entry per action submitted.

```python
from bulk_api.messages import OrderResponse

raw = client.place_orders([...])
responses = OrderResponse.from_api(raw)

for resp in responses:
    if resp.is_error():
        print(f"rejected: {resp.message}")
    else:
        print(f"oid={resp.order_id} status={resp.status}")
```

### `OrderResponse` fields

| Field | Type | Description |
|---|---|---|
| `order_id` | `Optional[str]` | Base58 order ID (present for placements) |
| `status` | `OrderStatus` | See status enum below |
| `message` | `Optional[str]` | Error detail when status is an error variant |
| `meta` | `dict` | Raw response body from the exchange |

### `OrderStatus` enum

| Value | String | Meaning |
|---|---|---|
| `RESTING` | `"resting"` / `"placed"` | Limit order is live on the book |
| `WORKING` | `"working"` | Order is being processed |
| `FILLED` | `"filled"` | Fully filled immediately |
| `PARTIALLY_FILLED` | `"partiallyFilled"` | Partially filled, remainder resting |
| `CANCELLED` | `"cancelled"` | Cancelled normally |
| `CANCELLED_IOC` | `"cancelledIOC"` | IOC order cancelled (unfilled remainder) |
| `CANCELLED_RISKLIMIT` | `"cancelledRiskLimit"` | Cancelled by risk engine |
| `CANCELLED_SELFCROSSING` | `"cancelledSelfCrossing"` | Cancelled to prevent self-cross |
| `CANCELLED_REDUCEONLY` | `"cancelledReduceOnly"` | Cancelled: reduce-only constraint |
| `REJECTED_CROSSING` | `"rejectedCrossing"` | Would cross own resting orders |
| `REJECTED_DUPLICATE` | `"rejectedDuplicate"` | Duplicate order ID |
| `REJECTED_RISKLIMIT` | `"rejectedRiskLimit"` | Would exceed risk/leverage limits |
| `REJECTED_INVALID` | `"rejectedInvalid"` | Malformed or invalid parameters |
| `CANCEL_REJECT` | `"cancelAllRejected"` / `"cancelOneRejected"` | Cancel request itself was rejected |
| `ERROR` | `"error"` | Generic exchange error |

### Helper methods

```python
resp.is_error()    # True for ERROR, REJECTED_* variants
resp.status.is_terminal()    # True if the order can no longer change state
resp.status.is_cancelled()   # True for any CANCELLED_* variant
resp.status.is_placed()      # True if the order is live (RESTING or WORKING)
```

---

## 8. Full Examples

### Read-only: market snapshot

```python
from bulk_api import BulkHttpClient

client = BulkHttpClient()

ticker = client.get_ticker("BTC-USD")
print(f"BTC mark={ticker['markPrice']} funding={ticker['fundingRate']:.6f}")

book = client.get_orderbook("BTC-USD", nlevels=5)
bids, asks = book["levels"]
print(f"best bid={bids[0][0]} best ask={asks[0][0]}")

candles = client.get_klines("BTC-USD", "1h", limit=5)
print(f"latest close={candles[-1]['c']}")
```

### Authenticated: place a ladder and cancel all on error

```python
import os
from bulk_api import BulkHttpClient
from bulk_api.messages import LimitOrder, CancelAll, OrderResponse
from bulk_api.common import SignatureDomain, Side, TimeInForce

client = BulkHttpClient(
    private_key=os.environ["BULK_PRIVATE_KEY"],
    signature_domain=SignatureDomain.MAINNET,
)

# Ladder: three bids at descending price levels
raw = client.place_orders([
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=95_000.0, size=0.05),
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=94_000.0, size=0.05),
    LimitOrder(symbol="BTC-USD", side=Side.BUY, price=93_000.0, size=0.05),
])

responses = OrderResponse.from_api(raw)

if any(r.is_error() for r in responses):
    print("One or more placements failed — cancelling all")
    for r in responses:
        if r.is_error():
            print(f"  {r.message}")
    client.place_orders([CancelAll(symbols=["BTC-USD"])])
else:
    for r in responses:
        print(f"placed oid={r.order_id} status={r.status}")
```

---

## 9. Comparison with the Rust HTTP Client

| Feature | Python | Rust |
|---|---|---|
| Construction | `BulkHttpClient(base_url, private_key, signature_domain, timeout)` | `BulkHttpClient::with_url(base_url, private_key, signature_domain)` or `BulkHttpClient::new(&config)` |
| Action types | `LimitOrder`, `MarketOrder`, `CancelOrder`, `CancelAll` dataclasses | `Action` enum variants |
| Batch submit | `place_orders(txns)` | `place_tx(actions, account, nonce)` |
| Single order helpers | — (use `place_orders` with one item) | `place_limit_order()`, `place_market_order()` |
| Account override | — (always uses signer's key) | `account: Option<Pubkey>` on every trading method |
| Nonce override | `nonce` param on `place_orders` only | `nonce: Option<u64>` on every trading method |
| Response type | `OrderResponse` (parsed via `from_api()`) | `Response` (returned directly) |
| `OrderStatus` | Rich enum with terminal/cancelled/placed helpers | String-based `status` field |
| Leverage input | `List[Tuple[str, float]]` | `HashMap<String, f64>` |
| Async | No (blocking `requests`) | Yes (`tokio` / `async fn`) |
