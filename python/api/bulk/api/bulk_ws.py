"""
Bulk Labs WebSocket Trading Client
Comprehensive WebSocket interface for market data and trading operations
"""

import asyncio
import json
import logging
import time
from argparse import Action
from typing import Dict, List, Optional, Callable, Any, Set, Union
from enum import Enum

import picows
from picows import WSFrame, WSTransport, WSListener, ws_connect, WSMsgType

from bulk.common import Side, TimeInForce, LoggingWebSocket
from bulk.common.inventory import Inventory, Pnl
from bulk.common.signer import TransactionSigner
from bulk.messages import SubscriptionRequest
from bulk.messages.account import AccountSnapshot, Margin, \
    LeverageSetting, MarginUpdate, PositionUpdate, OrderState
from bulk.messages.md import Ticker, Trade, L2Snapshot, L2Delta, Candle
from bulk.messages.trade import OrderResponse, Fill, CancelOrder, LimitOrder, CancelAll, MarketOrder, OraclePrices
from bulk.data import OrderBook
from bulk.common import Topic


class ConnectionState(Enum):
    """WebSocket connection states"""
    DISCONNECTED = 0
    CONNECTING = 1
    CONNECTED = 2
    RECONNECTING = 3


class BulkWSListener(WSListener):
    def __init__(self, client: 'BulkWebSocketClient'):
        self.client = client
        self.transport: Optional[WSTransport] = None

    def on_ws_connected(self, transport: WSTransport):
        self.transport = transport
        self.client.logger.info("picows connected")

    def on_ws_frame(self, transport: WSTransport, frame: WSFrame):
        if frame.msg_type == WSMsgType.TEXT:
            message = frame.get_payload_as_ascii_text()
            # Schedule handling on the event loop
            asyncio.get_event_loop().create_task(
                self.client._handle_raw_message(message)
            )
        elif frame.msg_type == WSMsgType.CLOSE:
            self.client.logger.warning("WebSocket close frame received")

    def on_ws_disconnected(self, transport: WSTransport):
        self.client.logger.warning("picows disconnected")
        asyncio.get_event_loop().create_task(self.client._reconnect())


class BulkWebSocketClient:
    """
    WebSocket client for Bulk Labs exchange
    Properly integrated with existing message type classes

    Features:
    - Emits typed events using existing message classes
    - Maintains OrderBook instances using your OrderBook class
    - Uses Inventory class for position tracking
    - All messages parsed with proper from_api methods

    Notes:
    - the following streams are auto-subscribed: ticker, account
    """

    def __init__(
        self,
        url: str = "wss://exchange-wss.bulk.trade",
        symbols: List[str] = ["BTC-USD", "ETH-USD", "SOL-USD"],
        signer: Optional[TransactionSigner] = None,
        inventory: Optional[Inventory] = None,
        logger: Optional[logging.Logger] = None,
        handlers: Optional[Dict[Topic,Callable]] = None,
        debug: bool = False,
    ):
        """
        Initialize WebSocket client

        Args:
            url: WebSocket endpoint URL
            symbols: List of symbols to subscribe to
            signer: TransactionSigner for signing orders
            inventory: Inventory instance for position tracking
            logger: Logger instance
        """
        self.url = url
        self.symbols = symbols
        self.signer = signer
        self.inventory = inventory or Inventory()
        self.logger = logger or logging.getLogger(__name__)

        # Connection management
        self.debug = debug
        self.t_backend_start = None
        self.ws: Optional[WSTransport] = None
        self.protocol: Optional[BulkWSListener] = None
        self.state = ConnectionState.DISCONNECTED
        self.reconnect_delay = 1.0
        self.max_reconnect_delay = 30.0
        self.reconnect_attempts = 0

        # Subscription tracking
        self.subscriptions: List[SubscriptionRequest] = []
        self.active_topics: Set[str] = set()

        # Event handlers - keyed by event type
        # Handler receives typed objects, not raw dicts
        self.handlers: Dict[str, List[Callable]] = {}

        # Order book management - uses your OrderBook class
        self.order_books: Dict[str, OrderBook] = {}

        # Latest ticker for each symbol
        self.tickers: Dict[str, Ticker] = {}
        self.prices: Dict[str, float] = {}

        # Open orders tracking
        self.open_orders: Dict[str, OrderState] = {}

        # Account state - uses your typed classes
        self.account_snapshot: Optional[AccountSnapshot] = None
        self.margin: Optional[Margin] = None
        self.leverage_settings: Dict[str, LeverageSetting] = {}

        # Request ID for post requests
        self.request_id = 0

        self.pending_requests: Dict[int, asyncio.Future] = {}
        self.default_timeout = 10.0  # seconds

        # Tasks
        self.receive_task: Optional[asyncio.Task] = None

        # Install handlers
        for etype, handler in handlers.items() if handlers is not None else {}:
            self.handlers[etype] = [handler]

    # ==================== CONNECTION MANAGEMENT ====================

    async def connect(self) -> bool:
        """
        Establish WebSocket connection

        Returns:
            True if connection successful
        """
        try:
            self.state = ConnectionState.CONNECTING
            self.logger.info(f"Connecting to {self.url}")

            (self.ws, self.protocol) = await ws_connect(
                lambda: BulkWSListener(self),  # factory function
                self.url,
                disconnect_on_exception=True,
            )

            self.t_backend_start = None
            self.state = ConnectionState.CONNECTED
            self.reconnect_delay = 1.0
            self.reconnect_attempts = 0
            self.logger.info("Connected to Bulk Exchange WebSocket")

            # Resubscribe to previous subscriptions
            if self.subscriptions:
                self.logger.info(f"Resubscribing to {len(self.subscriptions)} topics")
                await self._subscribe(self.subscriptions.copy(), resubscription=True)

            # subscribe to account and tickers
            else:
                if self.signer is not None:
                    await self.subscribe_account(self.signer.public_key)

                for symbol in self.symbols:
                    await self.subscribe_ticker(symbol)

            return True

        except Exception as e:
            self.logger.error(f"Connection failed: {e}")
            self.state = ConnectionState.DISCONNECTED
            return False

    async def disconnect(self):
        """Gracefully disconnect from WebSocket"""
        self.logger.info("Disconnecting from WebSocket")

        if self.receive_task:
            self.receive_task.cancel()
            try:
                await self.receive_task
            except asyncio.CancelledError:
                pass

        if self.ws:
            self.ws.send_close(1000)
            self.ws.disconnect()

        self.state = ConnectionState.DISCONNECTED
        self.ws = None
        self.protocol = None

    @property
    def is_connected(self) -> bool:
        """Check if WebSocket is connected"""
        return self.state == ConnectionState.CONNECTED and self.ws and not self.ws.is_closing

    # ==================== ACCESS METHODS ====================

    def get_book(self, symbol: str) -> Optional[OrderBook]:
        """Get OrderBook instance for a symbol"""
        return self.order_books.get(symbol)

    def get_ticker(self, symbol: str) -> Optional[Ticker]:
        """Get latest Ticker for a symbol"""
        return self.tickers.get(symbol)

    def get_orders(self, symbol: Optional[str] = None) -> List[OrderState]:
        """Get open orders, optionally filtered by symbol"""
        if symbol:
            return [
                order for order in self.open_orders.values()
                if order.symbol == symbol
            ]
        else:
            return list(self.open_orders.values())

    def get_pnl(self) -> Pnl:
        """Calculate P&L using Inventory class"""
        return self.inventory.pv(self.prices)

    # ==================== SUBSCRIPTIONS ====================

    async def subscribe_ticker(self, symbol: str):
        """Subscribe to ticker updates"""
        sub = SubscriptionRequest("ticker", {"symbol": symbol})
        await self._subscribe([sub])

    async def subscribe_trades(self, symbols: List[str]):
        """Subscribe to trades for symbols"""
        subs = [SubscriptionRequest("trades", {"symbol": sym}) for sym in symbols]
        await self._subscribe(subs)

    async def subscribe_orderbook_snapshot(self, symbol: str, nlevels: Optional[int] = None):
        """Subscribe to orderbook snapshots and initialize OrderBook"""
        params = {"symbol": symbol}
        if nlevels:
            params["nlevels"] = nlevels

        sub = SubscriptionRequest("l2Snapshot", params)
        await self._subscribe([sub])

        # Initialize order book using your OrderBook class
        if symbol not in self.order_books:
            self.order_books[symbol] = OrderBook(symbol)

    async def subscribe_orderbook_delta(self, symbol: str):
        """Subscribe to orderbook delta updates"""
        sub = SubscriptionRequest("l2Delta", {"symbol": symbol})
        await self._subscribe([sub])

    async def subscribe_account(self, user_pubkey: str):
        """Subscribe to account updates"""
        sub = SubscriptionRequest("account", {"user": user_pubkey})
        await self._subscribe([sub])

    async def subscribe_candles(self, symbol: str, interval: str):
        """Subscribe to candle updates"""
        sub = SubscriptionRequest("candle", {"symbol": symbol, "interval": interval})
        await self._subscribe([sub])


    # ==================== ORACLE ====================

    async def update_oracle(
        self,
        action: OraclePrices,
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ):
        """
        Update oracle prices
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")
        if not self.is_connected:
            raise RuntimeError("Not connected to WebSocket")

        if nonce:
            action.nonce = nonce

        tx = {
            "action": {
                "type": "oracle",
                "oracles": action.to_api(),
                "nonce": nonce,
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key,
        }
        tx = self.signer.sign_transaction(tx)

        # Build WebSocket request
        request = {
            "method": "post",
            "request": {
                "type": "action",
                "payload": tx
            },
            "id": self.request_id
        }

        # Create future for this request
        try:
            sjson = json.dumps(request)
            self.logger.debug(f"Sending request: {sjson}")
            self.ws.send(WSMsgType.TEXT, sjson.encode())
        except asyncio.TimeoutError:
            self.logger.error(f"Oracle post request timed out")
            raise
        except Exception as e:
            self.logger.error(f"Oracle placement error: {e}")
            raise


    # ==================== ORDERS ====================

    async def place_orders(
        self,
        actions: List[Union[LimitOrder|MarketOrder|CancelAll|CancelOrder]],
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ) -> List[OrderResponse]:
        """
        Place multiple orders and/or cancels
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")
        if not self.is_connected:
            raise RuntimeError("Not connected to WebSocket")

        self.request_id += 1
        request_id = self.request_id
        timeout = timeout if timeout is not None else self.default_timeout

        # Build transaction using LimitOrder.to_tx
        orders = []
        for action in actions:
            orders.append(action.to_api())

        # Bundle
        if not nonce:
            if actions[0].nonce:
                nonce = actions[0].nonce
            else:
                nonce = int(time.time_ns() / 1000)

        tx = tx = {
            "action": {
                "type": "order",
                "orders": orders,
                "nonce": nonce,
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key,
        }
        tx = self.signer.sign_transaction(tx)

        # Build WebSocket request
        request = {
            "method": "post",
            "request": {
                "type": "action",
                "payload": tx
            },
            "id": self.request_id
        }

        # Create future for this request
        future = asyncio.Future()
        self.pending_requests[request_id] = future

        try:
            sjson = json.dumps(request)
            #self.logger.debug(f"Sending request: {sjson}")

            self.t_backend_start = time.perf_counter()
            self.ws.send(WSMsgType.TEXT, sjson.encode())

            # Wait for response with timeout
            responses = await asyncio.wait_for(future, timeout=timeout)

            return responses
        except asyncio.TimeoutError:
            self.logger.error(f"Order request {request_id} timed out")
            self.pending_requests.pop(request_id, None)
            raise
        except Exception as e:
            self.logger.error(f"Order placement error: {e}")
            self.pending_requests.pop(request_id, None)
            raise

    async def place_limit_order(
        self,
        symbol: str,
        side: Side,
        price: float,
        size: float,
        reduce_only: bool = False,
        time_in_force: TimeInForce = TimeInForce.GTC,
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ) -> OrderResponse:
        """
        Place a limit order

        Args:
            symbol: Market symbol
            side: Buy or Sell
            price: Limit price
            size: Order size
            reduce_only: Reduce-only flag
            time_in_force: Time in force
            timeout: response timeout

        Returns:
            Order response
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1
        request_id = self.request_id
        timeout = timeout if timeout is not None else self.default_timeout

        # Create LimitOrder using your class
        limit_order = LimitOrder(
            symbol=symbol,
            side=side,
            price=price,
            size=size,
            reduce_only=reduce_only,
            time_in_force=time_in_force
        )

        results = await self.place_orders([limit_order], timeout=timeout, nonce=nonce)
        return results[0]

    async def place_market_order(
        self,
        symbol: str,
        side: Side,
        size: float,
        reduce_only: bool = False,
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ) -> OrderResponse:
        """
        Place a market order

        Args:
            symbol: Market symbol
            side: buy or sell
            size: Order size
            reduce_only: Reduce-only flag

        Returns:
            Order response
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1
        request_id = self.request_id
        timeout = timeout if timeout is not None else self.default_timeout

        # Import MarketOrder here to avoid circular imports
        from bulk.messages.trade import MarketOrder

        # Create MarketOrder using your class
        market_order = MarketOrder(
            symbol=symbol,
            side=side,
            size=size,
            reduce_only=reduce_only
        )

        # Build transaction using MarketOrder.to_tx
        results = await self.place_orders([market_order], timeout=timeout, nonce=nonce)
        return results[0]

    async def cancel_order(
        self,
        side: Side,
        symbol: str,
        order_id: str,
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ) -> OrderResponse:
        """
        Cancel an order

        Args:
            symbol: Market symbol
            order_id: Order ID to cancel
            timeout: response timeout

        Returns:
            Cancel response
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1
        request_id = self.request_id
        timeout = timeout if timeout is not None else self.default_timeout

        # Create CancelOrder using your class
        cancel_order = CancelOrder(symbol=symbol, order_id=order_id, side=side)
        # Build transaction using CancelOrder.to_tx
        results = await self.place_orders([cancel_order], timeout=timeout, nonce=nonce)
        return results[0]

    async def cancel_all(
        self,
        symbols: Optional[List[str]] = None,
        timeout: Optional[float] = None,
        nonce: Optional[int] = None,
    ) -> int:
        """
        Cancel an order

        Args:
            symbol: Market symbol
            order_id: Order ID to cancel
            timeout: response timeout

        Returns:
            Cancel response
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1
        request_id = self.request_id
        timeout = timeout if timeout is not None else self.default_timeout

        # Create CancelOrder using your class
        cancel_order = CancelAll(symbols=symbols)
        # Build transaction using CancelOrder.to_tx
        results = await self.place_orders([cancel_order], timeout=timeout, nonce=nonce)
        return results[0]

    # ==================== EVENT HANDLERS ====================

    def on(self, event_type: Topic, handler: Callable):
        """
        Register an event handler

        Event types and their emitted objects:
        - "ticker": Ticker
        - "trades": List[Trade]
        - "l2_snapshot": L2Snapshot
        - "l2_delta": L2Delta
        - "candle": Candle
        - "account_snapshot": AccountSnapshot
        - "margin_update": MarginUpdate
        - "position_update": PositionUpdate
        - "order": OrderState
        - "fill": Fill
        - "leverage_update": List[LeverageSetting]

        Args:
            event_type: Event type to handle
            handler: Callback function (can be sync or async)
        """
        if event_type not in self.handlers:
            self.handlers[event_type] = []
        self.handlers[event_type].append(handler)

    def off(self, event_type: Topic, handler: Callable):
        """Remove an event handler"""
        if event_type in self.handlers and handler in self.handlers[event_type]:
            self.handlers[event_type].remove(handler)


    # ==================== CONNECTION HANDLING ====================

    async def _reconnect(self):
        """Reconnect with exponential backoff"""
        self.state = ConnectionState.RECONNECTING
        self.reconnect_attempts += 1

        try:
            await self.ws.close()
        except Exception:
            pass

        delay = min(
            self.reconnect_delay * (2 ** min(self.reconnect_attempts - 1, 5)),
            self.max_reconnect_delay
        )

        self.logger.info(f"Reconnecting in {delay:.1f}s (attempt {self.reconnect_attempts})")
        await asyncio.sleep(delay)
        await self.connect()

    async def _handle_raw_message(self, message: str):
        """Called from listener's on_ws_frame"""
        try:
            if self.t_backend_start:
                t_backend_ms = (time.perf_counter() - self.t_backend_start) * 1000.0
                data = json.loads(message)
                if data["type"] == "post":
                    self.logger.info(f"Backend ms={t_backend_ms}, msg len: {len(message)}")
                    self.t_backend_start = None

            data = json.loads(message)
            await self._handle_message(data)
        except json.JSONDecodeError as e:
            self.logger.error(f"JSON decode error: {e}")
        except Exception as e:
            self.logger.error(f"Message handling error: {e}", exc_info=True)

    # ==================== SUBSCRIPTION MANAGEMENT ====================

    async def _subscribe(
        self,
        subscriptions: List[SubscriptionRequest],
        resubscription = False
    ) -> List[str]:
        """
        Subscribe to one or more topics

        Args:
            subscriptions: List of subscription requests

        Returns:
            List of active topic strings
        """
        if not self.ws or self.state != ConnectionState.CONNECTED:
            raise RuntimeError("Not connected to WebSocket")


        request = {
            "method": "subscribe",
            "subscription": [sub.to_dict() for sub in subscriptions]
        }

        self.ws.send(WSMsgType.TEXT, json.dumps(request).encode())

        # Store subscriptions for reconnection
        if not resubscription:
            self.subscriptions.extend(subscriptions)

        self.logger.info(f"Subscribed to {len(subscriptions)} topics")
        return list(self.active_topics)

    async def _unsubscribe(self, topic: str):
        """Unsubscribe from a specific topic"""
        if not self.ws or self.state != ConnectionState.CONNECTED:
            raise RuntimeError("Not connected to WebSocket")

        request = {
            "method": "unsubscribe",
            "topic": topic
        }

        self.ws.send(WSMsgType.TEXT, json.dumps(request).encode())
        self.active_topics.discard(topic)
        self.logger.info(f"Unsubscribed from {topic}")

    # ==================== MESSAGE HANDLING ====================

    async def _handle_message(self, data: Dict):
        """Route incoming messages to appropriate handlers"""
        msg_type = data.get("type")
        match msg_type:
            case "subscriptionResponse":
                self._handle_subscription_response(data)
            case "ticker":
                await self._handle_ticker(data)
            case "trades":
                await self._handle_trades(data)
            case "l2Snapshot":
                await self._handle_l2_snapshot(data)
            case "l2Delta":
                await self._handle_l2_delta(data)
            case "candle":
                await self._handle_candle(data)
            case "account":
                await self._handle_account_update(data)
            case "post":
                await self._handle_post_response(data)
            case _:
                self.logger.debug(f"Unhandled message type: {msg_type}")

    def _handle_subscription_response(self, data: Dict):
        """Handle subscription confirmation"""
        topics = data.get("topics", [])
        self.active_topics.update(topics)
        self.logger.info(f"Subscription confirmed for {len(topics)} topics")

    async def _handle_ticker(self, data: Dict):
        """
        Handle ticker update
        Emits: Ticker object
        """
        ticker_data = data.get("data", {}).get("ticker", {})

        # Parse using Ticker.from_api
        ticker = Ticker.from_api(ticker_data)

        # Store latest ticker
        self.tickers[ticker.symbol] = ticker
        self.prices[ticker.symbol] = ticker.mark_price

        # Emit typed event
        await self._emit_event(Topic.TICKER, ticker)

        price = ticker.last_price or 0.0
        self.logger.debug(
            f"Ticker: {ticker.symbol} "
            f"Last={price:.2f} "
            f"Change={ticker.price_change_percent:.2f}%"
        )

    async def _handle_trades(self, data: Dict):
        """
        Handle trade updates
        Emits: List[Trade] objects
        """
        # Parse using Trade.from_api which returns List[Trade]
        trades = Trade.from_api(data.get("data", {}))

        for trade in trades:
            # Check if this is our trade and update inventory
            if self._is_our_trade(trade):
                side_value = 1 if trade.side == Side.BUY else -1
                self.inventory.traded(
                    trade.symbol,
                    side_value,
                    trade.size,
                    trade.price
                )
                self.logger.info(
                    f"Our trade: {trade.symbol} "
                    f"{trade.side} {trade.size} @ {trade.price}"
                )

        # Emit typed events
        await self._emit_event(Topic.TRADES, trades)

        self.logger.debug(f"Trades: {len(trades)} trades for {trades[0].symbol if trades else 'unknown'}")

    async def _handle_l2_snapshot(self, data: Dict):
        """
        Handle order book snapshot
        Emits: L2Snapshot object
        """
        # Parse using L2Snapshot.from_api
        snapshot = L2Snapshot.from_api(data.get("data", {}))

        # Update OrderBook using your OrderBook class
        if snapshot.symbol in self.order_books:
            self.order_books[snapshot.symbol].from_snapshot(snapshot)
        else:
            # Initialize if not exists
            book = OrderBook(snapshot.symbol)
            book.from_snapshot(snapshot)
            self.order_books[snapshot.symbol] = book

        # Emit typed event
        await self._emit_event(Topic.L2SNAPSHOT, snapshot)

        book = self.order_books[snapshot.symbol]
        self.logger.debug(
            f"Book Snapshot: {snapshot.symbol} "
            f"Bid={book.get_best_bid().price if book.get_best_bid() else 0:.2f} "
            f"Ask={book.get_best_ask().price if book.get_best_ask() else 0:.2f}"
        )

    async def _handle_l2_delta(self, data: Dict):
        """
        Handle order book delta
        Emits: L2Delta object
        """
        # Parse using L2Delta.from_api
        delta = L2Delta.from_api(data.get("data", {}))

        # Apply delta to OrderBook
        if delta.symbol in self.order_books:
            self.order_books[delta.symbol].apply_delta(delta)
        else:
            self.logger.warning(f"Received delta for uninitialized book: {delta.symbol}")

        # Emit typed event
        await self._emit_event(Topic.L2DELTA, delta)

        self.logger.info(
            f"Book Delta: {delta.symbol} "
            f"Bid changes={len(delta.bid_changes)} "
            f"Ask changes={len(delta.ask_changes)}"
        )

    async def _handle_candle(self, data: Dict):
        """
        Handle candle update
        Emits: Candle object
        """
        candle_data = data.get("data", {}).get("candle", {})
        symbol = data.get("topic", "").split(".")[1] if "." in data.get("topic", "") else ""
        interval = data.get("topic", "").split(".")[2] if data.get("topic", "").count(".") >= 2 else "1m"

        # Parse using Candle.from_api
        candle = Candle.from_api(symbol, interval, candle_data)

        # Emit typed event
        await self._emit_event(Topic.CANDLE, candle)

        self.logger.debug(f"Candle: {candle.symbol} {candle.interval} Close={candle.close}")

    async def _handle_account_update(self, data: Dict):
        """Handle account updates - emits typed objects"""
        account_data = data.get("data", {})
        update_type = account_data.get("type")
        match update_type:
            case "accountSnapshot":
                await self._handle_account_snapshot(account_data)
            case "orderUpdate":
                await self._handle_order_update(account_data)
            case "marginUpdate":
                await self._handle_margin_update(account_data)
            case "positionUpdate":
                await self._handle_position_update(account_data)
            case "fill":
                await self._handle_fill(account_data)
            case "leverageUpdate":
                await self._handle_leverage_update(account_data)

    async def _handle_account_snapshot(self, data: Dict):
        """
        Handle account snapshot
        Emits: AccountSnapshot object
        """
        # Parse using AccountSnapshot.from_api
        snapshot = AccountSnapshot.from_api(data)

        # Store state
        self.account_snapshot = snapshot
        self.margin = snapshot.margin

        # Update positions
        for pos in snapshot.positions:
            position = self.inventory.position_for(pos.symbol)
            position.quantity = pos.size
            position.vwap = pos.price
            position.fair_price = pos.fair_price
            position.realized_pnl = pos.realized_pnl
            position.unrealized_pnl = pos.unrealized_pnl
            position.mm = pos.maintenance_margin
            position.mmr = pos.lambda_
            position.margin = pos.risk_allocation

        # Update open orders
        for order in snapshot.open_orders:
            self.open_orders[order.order_id] = order

        # Update leverage settings
        for lev in snapshot.leverage_settings:
            self.leverage_settings[lev.symbol] = lev

        # Emit typed event
        await self._emit_event(Topic.ACCOUNT, snapshot)

        self.logger.info(
            f"Account Snapshot: "
            f"Balance={snapshot.margin.total_balance:.2f} "
            f"Positions={len(snapshot.positions)} "
            f"Orders={len(snapshot.open_orders)}"
        )

    async def _handle_margin_update(self, data: Dict):
        """
        Handle margin update
        Emits: MarginUpdate object
        """
        # Parse using MarginUpdate.from_api
        margin_update = MarginUpdate.from_api(data)

        # Update margin (convert MarginUpdate to Margin for storage)
        self.margin = Margin.from_api(data)

        # Emit typed event
        await self._emit_event(Topic.MARGIN, margin_update)

        self.logger.debug(
            f"Margin: Balance={margin_update.total_balance:.2f} "
            f"Available={margin_update.available_balance:.2f}"
        )

    async def _handle_position_update(self, data: Dict):
        """
        Handle position update
        Emits: PositionUpdate object
        """
        # Parse using PositionUpdate.from_api
        pos = PositionUpdate.from_api(data)

        position = self.inventory.position_for(pos.symbol)
        position.quantity = pos.size
        position.vwap = pos.price
        position.fair_price = pos.fair_price
        position.realized_pnl = pos.realized_pnl
        position.unrealized_pnl = pos.unrealized_pnl
        position.mm = pos.maintenance_margin
        position.mmr = pos.lambda_
        position.margin = pos.risk_allocation

        # Emit typed event
        await self._emit_event(Topic.POSITION, pos)

    async def _handle_order_update(self, data: Dict):
        """
        Handle order status update
        Emits: OpenOrder object for placed, order_id for cancelled
        """
        state = OrderState.from_api(data)
        await self._emit_event(Topic.ORDER, state)


    async def _handle_fill(self, data: Dict):
        """
        Handle fill (trade execution)
        Emits: Fill object
        """
        # Parse using Fill.from_api
        fill = Fill.from_api(data)

        # Update inventory using your Inventory class
        side_value = 1 if fill.side == Side.BUY else -1
        self.inventory.traded(fill.symbol, side_value, fill.size, fill.price)

        # Update open orders
        if fill.order_id in self.open_orders:
            order = self.open_orders[fill.order_id]
            order.size -= fill.size
            order.size_done = order.size_done + fill.size

            if order.size <= 0:
                del self.open_orders[fill.order_id]

        # Emit typed event
        await self._emit_event(Topic.FILL, fill)

        self.logger.info(
            f"Fill: {fill.symbol} "
            f"{fill.side} {fill.size} @ {fill.price} "
            f"Maker={fill.is_maker}"
        )

    async def _handle_leverage_update(self, data: Dict):
        """
        Handle leverage update
        Emits: List[LeverageSetting] objects
        """
        leverage_list = data.get("leverage", [])

        # Parse leverage settings
        leverage_settings = [LeverageSetting.from_api(lev) for lev in leverage_list]

        # Update tracking
        for lev in leverage_settings:
            self.leverage_settings[lev.symbol] = lev

        # Emit typed event
        await self._emit_event(Topic.LEVERAGE, leverage_settings)
        self.logger.debug(f"Leverage updated for {len(leverage_settings)} symbols")

    async def _handle_post_response(self, data: Dict):
        """Handle response to order placement/cancellation"""
        request_id = data.get("id")
        response_data = data.get("data", {})
        payload = response_data.get("payload", {})
        status = payload.get("status")

        # Check if this is a pending request
        future = self.pending_requests.pop(request_id, None)

        if status != "ok":
            self.logger.error(f"Order request failed: {response_data}")
            if future and not future.done():
                # Set exception on future
                future.set_exception(RuntimeError(f"Order request failed: {response_data}"))
            await self._emit_event(Topic.ERROR, response_data)
        else:
            # Parse order responses
            self.logger.debug(f"Received response: {data}")
            order_responses = OrderResponse.from_api(data)

            # Resolve the future if it exists
            if future and not future.done():
                future.set_result(order_responses)

    async def _emit_event(self, event_type: Topic, event_data: Any):
        """Emit an event to registered handlers"""
        if event_type in self.handlers:
            for handler in self.handlers[event_type]:
                try:
                    if asyncio.iscoroutinefunction(handler):
                        await handler(event_data)
                    else:
                        handler(event_data)
                except Exception as e:
                    self.logger.error(
                        f"Handler error for {event_type}: {e}",
                        exc_info=True
                    )

    def _is_our_trade(self, trade: Trade) -> bool:
        """Check if a trade belongs to our account"""
        if not self.signer:
            return False
        return (
                trade.maker == self.signer.public_key or
                trade.taker == self.signer.public_key
        )

