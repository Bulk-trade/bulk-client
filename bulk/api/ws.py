"""
Bulk Labs WebSocket Trading Client
Comprehensive WebSocket interface for market data and trading operations
"""

import asyncio
import json
import logging
import time
from typing import Dict, List, Optional, Callable, Any, Set
from dataclasses import dataclass, field
from enum import Enum

import websockets
from websockets.legacy.client import WebSocketClientProtocol

from bulk.common import Side, TimeInForce, LoggingWebSocket
from bulk.common.inventory import Inventory, Pnl
from bulk.common.signer import TransactionSigner
from bulk.messages import SubscriptionRequest
from bulk.messages.account import OpenOrder, AccountSnapshot, Margin, LeverageSetting, MarginUpdate, PositionUpdate
from bulk.messages.md import OrderBook, Ticker, Trade, L2Snapshot, L2Delta, Candle
from bulk.messages.trade import Fill, CancelOrder, LimitOrder


class ConnectionState(Enum):
    """WebSocket connection states"""
    DISCONNECTED = 0
    CONNECTING = 1
    CONNECTED = 2
    RECONNECTING = 3


class BulkWebSocketClient:
    """
    WebSocket client for Bulk Labs exchange
    Properly integrated with existing message type classes

    Features:
    - Emits typed events using existing message classes
    - Maintains OrderBook instances using your OrderBook class
    - Uses Inventory class for position tracking
    - All messages parsed with proper from_api methods
    """

    def __init__(
        self,
        url: str = "wss://exchange-wss.bulk.trade",
        signer: Optional[TransactionSigner] = None,
        inventory: Optional[Inventory] = None,
        logger: Optional[logging.Logger] = None,
        handlers: Optional[Dict[str,Callable]] = None,
        debug: bool = False,
    ):
        """
        Initialize WebSocket client

        Args:
            url: WebSocket endpoint URL
            signer: TransactionSigner for signing orders
            inventory: Inventory instance for position tracking
            logger: Logger instance
        """
        self.url = url
        self.signer = signer
        self.inventory = inventory or Inventory()
        self.logger = logger or logging.getLogger(__name__)

        # Connection management
        self.debug = debug
        self.ws: Optional[WebSocketClientProtocol] = None
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

        # Open orders tracking
        self.open_orders: Dict[str, OpenOrder] = {}

        # Account state - uses your typed classes
        self.account_snapshot: Optional[AccountSnapshot] = None
        self.margin: Optional[Margin] = None
        self.leverage_settings: Dict[str, LeverageSetting] = {}

        # Request ID for post requests
        self.request_id = 0

        # Tasks
        self.receive_task: Optional[asyncio.Task] = None

        # Install handlers
        for etype, handler in self.handlers.items() if handlers is not None else {}:
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

            self.ws = await websockets.connect(
                self.url,
                ping_interval=20,
                ping_timeout=10
            )
            if self.debug:
                self.ws = LoggingWebSocket(self.ws, logger=self.logger)

            self.state = ConnectionState.CONNECTED
            self.reconnect_delay = 1.0
            self.reconnect_attempts = 0
            self.logger.info("Connected to Bulk Exchange WebSocket")

            # Start receive loop
            self.receive_task = asyncio.create_task(self._receive_loop())

            # Resubscribe to previous subscriptions
            if self.subscriptions:
                self.logger.info(f"Resubscribing to {len(self.subscriptions)} topics")
                await self.subscribe(self.subscriptions.copy())

            # we want to get account updates
            elif self.signer is not None:
                await self.subscribe_account(self.signer.public_key)

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
            await self.ws.close()

        self.state = ConnectionState.DISCONNECTED
        self.ws = None

    # ==================== SUBSCRIPTION MANAGEMENT ====================

    async def subscribe(self, subscriptions: List[SubscriptionRequest]) -> List[str]:
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

        await self.ws.send(json.dumps(request))

        # Store subscriptions for reconnection
        self.subscriptions.extend(subscriptions)

        self.logger.info(f"Subscribed to {len(subscriptions)} topics")
        return list(self.active_topics)

    async def unsubscribe(self, topic: str):
        """Unsubscribe from a specific topic"""
        if not self.ws or self.state != ConnectionState.CONNECTED:
            raise RuntimeError("Not connected to WebSocket")

        request = {
            "method": "unsubscribe",
            "topic": topic
        }

        await self.ws.send(json.dumps(request))
        self.active_topics.discard(topic)
        self.logger.info(f"Unsubscribed from {topic}")


    # ==================== MARKET DATA SUBSCRIPTIONS ====================

    async def subscribe_ticker(self, symbol: str):
        """Subscribe to ticker updates"""
        sub = SubscriptionRequest("ticker", {"symbol": symbol})
        await self.subscribe([sub])

    async def subscribe_trades(self, symbols: List[str]):
        """Subscribe to trades for symbols"""
        subs = [SubscriptionRequest("trades", {"symbol": sym}) for sym in symbols]
        await self.subscribe(subs)

    async def subscribe_orderbook_snapshot(self, symbol: str, nlevels: Optional[int] = None):
        """Subscribe to orderbook snapshots and initialize OrderBook"""
        params = {"symbol": symbol}
        if nlevels:
            params["nlevels"] = nlevels

        sub = SubscriptionRequest("l2Snapshot", params)
        await self.subscribe([sub])

        # Initialize order book using your OrderBook class
        if symbol not in self.order_books:
            self.order_books[symbol] = OrderBook(symbol)

    async def subscribe_orderbook_delta(self, symbol: str):
        """Subscribe to orderbook delta updates"""
        sub = SubscriptionRequest("l2Delta", {"symbol": symbol})
        await self.subscribe([sub])

    async def subscribe_account(self, user_pubkey: str):
        """Subscribe to account updates"""
        sub = SubscriptionRequest("account", {"user": user_pubkey})
        await self.subscribe([sub])

    async def subscribe_candles(self, symbol: str, interval: str):
        """Subscribe to candle updates"""
        sub = SubscriptionRequest("candle", {"symbol": symbol, "interval": interval})
        await self.subscribe([sub])

    # ==================== ORDER PLACEMENT ====================

    async def place_limit_order(
        self,
        symbol: str,
        is_buy: bool,
        price: float,
        size: float,
        reduce_only: bool = False,
        time_in_force: TimeInForce = TimeInForce.GTC
    ) -> int:
        """
        Place a limit order

        Args:
            symbol: Market symbol
            is_buy: True for buy, False for sell
            price: Limit price
            size: Order size
            reduce_only: Reduce-only flag
            time_in_force: Time in force

        Returns:
            Request ID
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1

        # Create LimitOrder using your class
        limit_order = LimitOrder(
            symbol=symbol,
            is_buy=is_buy,
            price=price,
            size=size,
            reduce_only=reduce_only,
            time_in_force=time_in_force
        )

        # Build transaction using LimitOrder.to_tx
        transaction = limit_order.to_tx(self.signer)

        # Build WebSocket request
        request = {
            "method": "post",
            "request": {
                "type": "action",
                "payload": transaction
            },
            "id": self.request_id
        }

        await self.ws.send(json.dumps(request))

        self.logger.info(
            f"Placed limit order: {symbol} "
            f"{'BUY' if is_buy else 'SELL'} {size} @ {price}"
        )

        return self.request_id

    async def place_market_order(
        self,
        symbol: str,
        side: Side,
        size: float,
        reduce_only: bool = False
    ) -> int:
        """
        Place a market order

        Args:
            symbol: Market symbol
            side: buy or sell
            size: Order size
            reduce_only: Reduce-only flag

        Returns:
            Request ID
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1

        # Import MarketOrder here to avoid circular imports
        from bulk.messages.trade import MarketOrder

        # Create MarketOrder using your class
        market_order = MarketOrder(
            symbol=symbol,
            is_buy=side == Side.BUY,
            size=size,
            reduce_only=reduce_only
        )

        # Build transaction using MarketOrder.to_tx
        transaction = market_order.to_tx(self.signer)

        # Build WebSocket request
        request = {
            "method": "post",
            "request": {
                "type": "action",
                "payload": transaction
            },
            "id": self.request_id
        }

        print(f"Placed market order: {json.dumps(request)} ")
        await self.ws.send(json.dumps(request))

        self.logger.info(
            f"Placed market order: {symbol} {side} @ MARKET"
        )
        return self.request_id

    async def cancel_order(self, symbol: str, order_id: str) -> int:
        """
        Cancel an order

        Args:
            symbol: Market symbol
            order_id: Order ID to cancel

        Returns:
            Request ID
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")

        self.request_id += 1

        # Create CancelOrder using your class
        cancel_order = CancelOrder(symbol=symbol, oid=order_id)

        # Build transaction using CancelOrder.to_tx
        transaction = cancel_order.to_tx(self.signer)

        # Build WebSocket request
        request = {
            "method": "post",
            "request": {
                "type": "action",
                "payload": transaction
            },
            "id": self.request_id
        }

        await self.ws.send(json.dumps(request))

        self.logger.info(f"Cancelled order: {order_id}")

        return self.request_id

    # ==================== EVENT HANDLERS ====================

    def on(self, event_type: str, handler: Callable):
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
        - "order_placed": OpenOrder
        - "order_cancelled": str (order_id)
        - "fill": Fill
        - "leverage_update": List[LeverageSetting]
        - "order_state": str (order_id)
        - "order_error": str

        Args:
            event_type: Event type to handle
            handler: Callback function (can be sync or async)
        """
        if event_type not in self.handlers:
            self.handlers[event_type] = []
        self.handlers[event_type].append(handler)

    def off(self, event_type: str, handler: Callable):
        """Remove an event handler"""
        if event_type in self.handlers and handler in self.handlers[event_type]:
            self.handlers[event_type].remove(handler)

    # ==================== UTILITY METHODS ====================

    def get_order_book(self, symbol: str) -> Optional[OrderBook]:
        """Get OrderBook instance for a symbol"""
        return self.order_books.get(symbol)

    def get_ticker(self, symbol: str) -> Optional[Ticker]:
        """Get latest Ticker for a symbol"""
        return self.tickers.get(symbol)

    def get_open_orders(self, symbol: Optional[str] = None) -> List[OpenOrder]:
        """Get open orders, optionally filtered by symbol"""
        if symbol:
            return [
                order for order in self.open_orders.values()
                if order.symbol == symbol
            ]
        return list(self.open_orders.values())

    def get_pnl(self, prices: Dict[str, float]) -> Pnl:
        """Calculate P&L using Inventory class"""
        return self.inventory.pv(prices)

    @property
    def is_connected(self) -> bool:
        """Check if WebSocket is connected"""
        return (
                self.state == ConnectionState.CONNECTED and
                self.ws and
                not self.ws.closed
        )

    # ==================== CONNECTION HANDLING ====================

    async def _reconnect(self):
        """Reconnect with exponential backoff"""
        self.state = ConnectionState.RECONNECTING
        self.reconnect_attempts += 1

        delay = min(
            self.reconnect_delay * (2 ** min(self.reconnect_attempts - 1, 5)),
            self.max_reconnect_delay
        )

        self.logger.info(f"Reconnecting in {delay:.1f}s (attempt {self.reconnect_attempts})")
        await asyncio.sleep(delay)

        await self.connect()

    async def _receive_loop(self):
        """Main message receive loop"""
        try:
            async for message in self.ws:
                try:
                    data = json.loads(message)
                    print(f"Received message: {data}")
                    await self._handle_message(data)
                except json.JSONDecodeError as e:
                    self.logger.error(f"JSON decode error: {e}")
                except Exception as e:
                    self.logger.error(f"Message handling error: {e}", exc_info=True)

        except websockets.exceptions.ConnectionClosed:
            self.logger.warning("WebSocket connection closed")
            await self._reconnect()
        except Exception as e:
            self.logger.error(f"Receive loop error: {e}", exc_info=True)
            await self._reconnect()


    # ==================== MESSAGE HANDLING ====================

    async def _handle_message(self, data: Dict):
        """Route incoming messages to appropriate handlers"""
        msg_type = data.get("type")
        if msg_type == "subscriptionResponse":
            self._handle_subscription_response(data)
        elif msg_type == "ticker":
            await self._handle_ticker(data)
        elif msg_type == "trades":
            await self._handle_trades(data)
        elif msg_type == "l2Snapshot":
            await self._handle_l2_snapshot(data)
        elif msg_type == "l2Delta":
            await self._handle_l2_delta(data)
        elif msg_type == "candle":
            await self._handle_candle(data)
        elif msg_type == "account":
            await self._handle_account_update(data)
        elif msg_type == "post":
            await self._handle_post_response(data)
        else:
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

        # Emit typed event
        await self._emit_event("ticker", ticker)

        self.logger.debug(
            f"Ticker: {ticker.symbol} "
            f"Last={ticker.last_price:.2f} "
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
        await self._emit_event("trades", trades)

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
        await self._emit_event("l2_snapshot", snapshot)

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
        await self._emit_event("l2_delta", delta)

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
        await self._emit_event("candle", candle)

        self.logger.debug(f"Candle: {candle.symbol} {candle.interval} Close={candle.close}")

    async def _handle_account_update(self, data: Dict):
        """Handle account updates - emits typed objects"""
        account_data = data.get("data", {})
        update_type = account_data.get("type")

        if update_type == "accountSnapshot":
            await self._handle_account_snapshot(account_data)
        elif update_type == "marginUpdate":
            await self._handle_margin_update(account_data)
        elif update_type == "positionUpdate":
            await self._handle_position_update(account_data)
        elif update_type == "order":
            await self._handle_order_update(account_data)
        elif update_type == "fill":
            await self._handle_fill(account_data)
        elif update_type == "leverageUpdate":
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
        await self._emit_event("account_snapshot", snapshot)

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
        await self._emit_event("margin_update", margin_update)

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
        await self._emit_event("position_update", pos)

    async def _handle_order_update(self, data: Dict):
        """
        Handle order status update
        Emits: OpenOrder object for placed, order_id for cancelled
        """
        order_id = data.get("orderId")
        status = data.get("status")

        if status == "placed":
            # Parse as OpenOrder
            order = OpenOrder.from_api(data)
            self.open_orders[order_id] = order

            # Emit typed event
            await self._emit_event("order_placed", order)

            self.logger.info(
                f"Order Placed: {order.symbol} "
                f"{'BUY' if order.is_buy else 'SELL'} "
                f"{order.size} @ {order.price}"
            )

        elif status == "cancelled":
            # Remove from tracking
            order = self.open_orders.pop(order_id, None)

            # Emit cancellation event with order_id
            await self._emit_event("order_cancelled", order_id)

            self.logger.info(f"Order Cancelled: {order_id}")

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
            order.filled_size = order.filled_size + fill.size

            if order.size <= 0:
                del self.open_orders[fill.order_id]

        # Emit typed event
        await self._emit_event("fill", fill)

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
        await self._emit_event("leverage_update", leverage_settings)

        self.logger.debug(f"Leverage updated for {len(leverage_settings)} symbols")

    async def _handle_post_response(self, data: Dict):
        """Handle response to order placement/cancellation"""
        response_data = data.get("data", {})
        payload = response_data.get("payload", {})
        status = payload.get("status")
        request_id = data.get("id")

        if status != "ok":
            self.logger.error(f"Order request failed: {response_data}")
            await self._emit_event("order_request_failed", response_data)
            return

        response = payload.get("response", {})
        response_type = response.get("type")

        if response_type == "order":
            order_data = response.get("data", {})
            statuses = order_data.get("statuses", [])

            for order_status in statuses:
                await self._emit_event("order_state", order_status)


    async def _emit_event(self, event_type: str, event_data: Any):
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

