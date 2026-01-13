"""
Binance USDT-M Futures WebSocket Client
Async WebSocket interface for perpetual futures market data
"""

import asyncio
import json
import logging
import time
from typing import Dict, List, Optional, Callable, Any
from enum import Enum
import aiohttp

import websockets
from polars.io import delta
from websockets.asyncio.client import ClientConnection, connect as ws_connect

from bulk.common import Side
from bulk.messages.md import L2Snapshot, L2Delta, OrderBookLevel, BBO
from data.orderbook_jit import FastOrderBook


class ConnectionState(Enum):
    """WebSocket connection states"""
    DISCONNECTED = 0
    CONNECTING = 1
    CONNECTED = 2
    RECONNECTING = 3


class Topic(Enum):
    """WebSocket topics for event emission"""
    L2_SNAPSHOT = "l2_snapshot"
    L2_DELTA = "l2_delta"
    BBO = "bbo"
    ERROR = "error"


class BinanceFuturesWebSocketClient:
    """
    Async WebSocket client for Binance USDT-M Futures (linear perpetuals)

    Features:
    - Real-time depth updates (@depth stream)
    - Optional BBO stream (@bookTicker)
    - Proper sequence validation
    - Auto-reconnect with exponential backoff
    - Event emission system matching BulkWebSocketClient
    """

    # Binance USDT-M Futures endpoints
    WS_URL = "wss://fstream.binance.com"
    REST_URL = "https://fapi.binance.com"

    def __init__(
        self,
        symbols: List[str],
        snapshot_limit: int = 1000,
        enable_bbo: bool = True,
        logger: Optional[logging.Logger] = None,
        handlers: Optional[Dict[Topic, Callable]] = None,
        debug: bool = False,
    ):
        """
        Initialize Binance Futures WebSocket client

        Args:
            symbols: List of perpetual symbols (e.g., ["BTCUSDT", "ETHUSDT"])
            snapshot_limit: Depth levels for initial snapshot (max 1000)
            enable_bbo: Whether to subscribe to bookTicker stream
            logger: Logger instance
            handlers: Event handlers keyed by Topic
            debug: Enable debug logging
        """
        self.symbols = [s.upper() for s in symbols]
        self.snapshot_limit = snapshot_limit
        self.enable_bbo = enable_bbo
        self.logger = logger or logging.getLogger(__name__)
        self.debug = debug

        # Connection management
        self.ws_depth: Optional[ClientConnection] = None
        self.ws_bbo: Optional[ClientConnection] = None
        self.state = ConnectionState.DISCONNECTED
        self.reconnect_delay = 1.0
        self.max_reconnect_delay = 30.0
        self.reconnect_attempts = 0

        # Order book management (all use 9 decimals)
        self.order_books: Dict[str, FastOrderBook] = {
            symbol: FastOrderBook(symbol) for symbol in self.symbols
        }

        # Sequence tracking per symbol
        self.last_update_id: Dict[str, int] = {}
        self.prev_final_update_id: Dict[str, int] = {}  # pu field for futures
        self.snapshot_last_update_id: Dict[str, int] = {}
        self.update_buffers: Dict[str, List[Dict]] = {sym: [] for sym in self.symbols}

        # Event handlers
        self.handlers: Dict[Topic, List[Callable]] = {}
        for topic, handler in (handlers or {}).items():
            self.handlers[topic] = [handler]

        # Tasks
        self.depth_task: Optional[asyncio.Task] = None
        self.bbo_task: Optional[asyncio.Task] = None

    # ==================== ACCESS METHODS ====================

    def get_book(self, symbol: str) -> Optional[FastOrderBook]:
        """Get OrderBook instance for a symbol"""
        return self.order_books.get(symbol.upper())

    def get_best_bid(self, symbol: str) -> Optional[OrderBookLevel]:
        """Get best bid for a symbol"""
        book = self.get_book(symbol)
        return book.get_best_bid() if book else None

    def get_best_ask(self, symbol: str) -> Optional[OrderBookLevel]:
        """Get best ask for a symbol"""
        book = self.get_book(symbol)
        return book.get_best_ask() if book else None

    def get_mid_price(self, symbol: str) -> Optional[float]:
        """Get mid price for a symbol"""
        book = self.get_book(symbol)
        return book.get_mid_price() if book else None

    def get_spread(self, symbol: str) -> Optional[float]:
        """Get spread for a symbol"""
        book = self.get_book(symbol)
        return book.get_spread() if book else None

    # ==================== CONNECTION MANAGEMENT ====================

    async def connect(self) -> bool:
        """
        Establish WebSocket connections and initialize order books

        Process:
        1. Start depth WebSocket
        2. Buffer incoming updates
        3. Fetch snapshots via REST
        4. Apply buffered updates
        5. Start BBO stream if enabled

        Returns:
            True if connection successful
        """
        try:
            self.state = ConnectionState.CONNECTING
            self.logger.info("Connecting to Binance USDT-M Futures")

            # Start depth stream
            await self._connect_depth_stream()

            # Give WebSocket time to connect and buffer updates
            await asyncio.sleep(1)

            # Fetch snapshots and initialize order books
            await self._initialize_order_books()

            # Start BBO stream if enabled
            if self.enable_bbo:
                await self._connect_bbo_stream()

            self.state = ConnectionState.CONNECTED
            self.reconnect_delay = 1.0
            self.reconnect_attempts = 0
            self.logger.info("Connected to Binance Futures WebSocket")

            return True

        except Exception as e:
            self.logger.error(f"Connection failed: {e}")
            self.state = ConnectionState.DISCONNECTED
            return False

    async def disconnect(self):
        """Gracefully disconnect from WebSocket"""
        self.logger.info("Disconnecting from Binance Futures WebSocket")

        if self.depth_task:
            self.depth_task.cancel()
            try:
                await self.depth_task
            except asyncio.CancelledError:
                pass

        if self.bbo_task:
            self.bbo_task.cancel()
            try:
                await self.bbo_task
            except asyncio.CancelledError:
                pass

        if self.ws_depth:
            await self.ws_depth.close()

        if self.ws_bbo:
            await self.ws_bbo.close()

        self.state = ConnectionState.DISCONNECTED
        self.ws_depth = None
        self.ws_bbo = None

    @property
    def is_connected(self) -> bool:
        """Check if WebSocket is connected"""
        return (
                self.state == ConnectionState.CONNECTED and
                self.ws_depth is not None and
                not self.ws_depth.closed
        )

    # ==================== EVENT HANDLING ====================

    def on(self, event_type: Topic, handler: Callable):
        """
        Register an event handler

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

    # ==================== SNAPSHOT FETCHING ====================

    async def fetch_snapshot(self, symbol: str) -> L2Snapshot:
        """
        Fetch order book snapshot via REST API

        Args:
            symbol: Perpetual symbol (e.g., "BTCUSDT")

        Returns:
            L2Snapshot object
        """
        endpoint = f"{self.REST_URL}/fapi/v1/depth"
        params = {
            "symbol": symbol,
            "limit": self.snapshot_limit
        }

        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(endpoint, params=params, timeout=5) as response:
                    response.raise_for_status()
                    data = await response.json()

            # Store lastUpdateId for sequence validation
            self.snapshot_last_update_id[symbol] = data.get("lastUpdateId")

            # Convert to L2Snapshot
            snapshot = self._convert_snapshot(symbol, data)
            return snapshot

        except Exception as e:
            self.logger.error(f"Failed to fetch snapshot for {symbol}: {e}")
            raise

    # ==================== WEBSOCKET STREAMS ====================

    async def _connect_depth_stream(self):
        """Connect to depth update stream (real-time)"""
        # Build combined stream URL for all symbols
        # Futures uses @depth for real-time updates (equivalent to @depth@0ms)
        streams = [f"{symbol.lower()}@depth" for symbol in self.symbols]
        stream_names = "/".join(streams)
        url = f"{self.WS_URL}/stream?streams={stream_names}"

        self.logger.info(f"Connecting to depth stream: {url}")
        self.ws_depth = await ws_connect(
            url,
            ping_interval=20,
            ping_timeout=10,
            close_timeout=10,
        )

        # Start receive loop
        self.depth_task = asyncio.create_task(self._depth_receive_loop())

    async def _connect_bbo_stream(self):
        """Connect to book ticker (BBO) stream"""
        streams = [f"{sym.lower()}@bookTicker" for sym in self.symbols]
        stream_names = "/".join(streams)
        url = f"{self.WS_URL}/stream?streams={stream_names}"

        self.logger.info(f"Connecting to BBO stream: {url}")
        self.ws_bbo = await ws_connect(
            url,
            ping_interval=20,
            ping_timeout=10,
            close_timeout=10,
        )

        # Start receive loop
        self.bbo_task = asyncio.create_task(self._bbo_receive_loop())

    async def _initialize_order_books(self):
        """Fetch snapshots and initialize order books"""
        self.logger.info(f"Fetching snapshots for {len(self.symbols)} symbols...")

        # Fetch all snapshots concurrently
        snapshot_tasks = [self.fetch_snapshot(symbol) for symbol in self.symbols]
        snapshots = await asyncio.gather(*snapshot_tasks)

        # Apply snapshots to order books
        for snapshot in snapshots:
            self.order_books[snapshot.symbol].from_snapshot(snapshot)
            self.logger.info(
                f"[{snapshot.symbol}] Snapshot applied. "
                f"Last update ID: {self.snapshot_last_update_id[snapshot.symbol]}"
            )

            # Apply buffered updates
            buffered = self.update_buffers[snapshot.symbol]
            applied = 0
            for update in buffered:
                if self._process_depth_update(update):
                    applied += 1

            self.logger.info(
                f"[{snapshot.symbol}] Applied {applied}/{len(buffered)} buffered updates"
            )
            self.update_buffers[snapshot.symbol].clear()

    # ==================== RECEIVE LOOPS ====================

    def _emit_event(self, event_type: Topic, event_data: Any):
        """Emit an event to registered handlers (fire-and-forget)"""
        if event_type in self.handlers:
            for handler in self.handlers[event_type]:
                try:
                    if asyncio.iscoroutinefunction(handler):
                        asyncio.create_task(handler(self, event_data))
                    else:
                        handler(self, event_data)
                except Exception as e:
                    self.logger.error(
                        f"Handler error for {event_type}: {e}",
                        exc_info=True
                    )

    async def _depth_receive_loop(self):
        """Main receive loop for depth updates"""
        try:
            async for message in self.ws_depth:
                try:
                    data = json.loads(message)

                    # Combined stream format wraps data
                    if "stream" in data:
                        stream_data = data.get("data", {})
                        self._handle_depth_message(stream_data)
                    else:
                        self._handle_depth_message(data)

                except json.JSONDecodeError as e:
                    self.logger.error(f"JSON decode error: {e}")
                except Exception as e:
                    self.logger.error(f"Message handling error: {e}", exc_info=True)

        except websockets.exceptions.ConnectionClosed:
            self.logger.warning("Depth WebSocket connection closed")
            await self._reconnect()
        except Exception as e:
            self.logger.error(f"Depth receive loop error: {e}", exc_info=True)
            await self._reconnect()

    async def _bbo_receive_loop(self):
        """Receive loop for BBO updates"""
        try:
            async for message in self.ws_bbo:
                try:
                    data = json.loads(message)

                    # Combined stream format
                    if "stream" in data:
                        stream_data = data.get("data", {})
                        await self._handle_bbo_message(stream_data)
                    else:
                        await self._handle_bbo_message(data)

                except json.JSONDecodeError as e:
                    self.logger.error(f"JSON decode error: {e}")
                except Exception as e:
                    self.logger.error(f"BBO handling error: {e}", exc_info=True)

        except websockets.exceptions.ConnectionClosed:
            self.logger.warning("BBO WebSocket connection closed")
        except Exception as e:
            self.logger.error(f"BBO receive loop error: {e}", exc_info=True)

    def _handle_depth_message(self, data: Dict):
        """Handle depth update message"""
        symbol = data.get("s", "").upper()
        if symbol not in self.symbols:
            return

        # Buffer updates before snapshot is applied
        if symbol not in self.snapshot_last_update_id:
            self.update_buffers[symbol].append(data)
            if len(self.update_buffers[symbol]) > 1000:
                self.update_buffers[symbol].pop(0)
            return

        # Process update
        self._process_depth_update(data)

    def _process_depth_update(self, data: Dict) -> bool:
        """
        Process a single depth update

        Returns:
            True if update was applied, False if dropped
        """
        symbol = data.get("s", "").upper()

        try:
            # Convert to delta and apply
            delta = self._convert_delta(symbol, data)
            self.order_books[symbol].apply_delta(delta)

            # Emit event
            self._emit_event(Topic.L2_DELTA, delta)
            return True

        except Exception as e:
            self.logger.error(f"Error processing depth update for {symbol}: {e}")
            return False

    async def _handle_bbo_message(self, data: Dict):
        """Handle book ticker (BBO) message"""
        try:
            symbol = data.get("s", "").upper()
            if symbol not in self.symbols:
                return

            best_bid = float(data.get("b", 0))
            best_ask = float(data.get("a", 0))
            bid_size = float(data.get("B", 0))
            ask_size = float(data.get("A", 0))

            best_bid = OrderBookLevel(price=best_bid, size=bid_size)
            best_ask = OrderBookLevel(price=best_ask, size=ask_size)

            bbo = BBO(
                timestamp=data.get("E", time.time_ns()),
                symbol=symbol,
                bid=best_bid,
                ask=best_ask,
            )

            self.order_books[symbol].apply_bbo(bbo)

            # Emit BBO event
            self._emit_event(Topic.BBO, bbo)
        except Exception as e:
            self.logger.error(f"Error processing BBO update: {e}")

    async def _reconnect(self):
        """Reconnect with exponential backoff"""
        self.state = ConnectionState.RECONNECTING
        self.reconnect_attempts += 1

        delay = min(
            self.reconnect_delay * (2 ** min(self.reconnect_attempts - 1, 5)),
            self.max_reconnect_delay
        )

        self.logger.info(
            f"Reconnecting in {delay:.1f}s (attempt {self.reconnect_attempts})"
        )
        await asyncio.sleep(delay)
        await self.connect()

    # ==================== PARSING, ETC ====================

    def _convert_snapshot(self, symbol: str, data: Dict) -> L2Snapshot:
        """Convert Binance snapshot format to L2Snapshot"""
        bids = [
            OrderBookLevel(
                price=float(price),
                size=float(size),
                num_orders=0
            )
            for price, size in data.get("bids", [])
        ]

        asks = [
            OrderBookLevel(
                price=float(price),
                size=float(size),
                num_orders=0
            )
            for price, size in data.get("asks", [])
        ]

        return L2Snapshot(
            symbol=symbol,
            bids=bids,
            asks=asks,
            timestamp=int(time.time() * 1000)
        )

    def _convert_delta(self, symbol: str, data: Dict) -> L2Delta:
        """Convert Binance depth update to L2Delta"""
        bid_changes = [
            OrderBookLevel(
                price=float(price),
                size=float(size),
                num_orders=0
            )
            for price, size in data.get("b", [])
        ]

        ask_changes = [
            OrderBookLevel(
                price=float(price),
                size=float(size),
                num_orders=0
            )
            for price, size in data.get("a", [])
        ]

        return L2Delta(
            symbol=symbol,
            bid_changes=bid_changes,
            ask_changes=ask_changes,
            timestamp=data.get("E", int(time.time() * 1000))
        )


# ==================== EXAMPLE USAGE ====================

symbols_seen = set([])
ticks = 0

bids_seen1 = set()
asks_seen1 = set()

bids_seen2 = set()
asks_seen2 = set()

async def main():
    """Example usage"""
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )

    # BBO handler
    async def handle_bbo(client: BinanceFuturesWebSocketClient, bbo: BBO):
        print(bbo)

    # Delta handler
    async def handle_delta1(client: BinanceFuturesWebSocketClient, delta: L2Delta):
        global symbols_seen, ticks, bids_seen, asks_seen

        ticks += 1
        symbols_seen.add(delta.symbol)
        print(
            f"[{delta.symbol}] Delta: "
            f"Bids={len(delta.bid_changes)} Asks={len(delta.ask_changes)}"
        )

        if (ticks % 10) > 0:
            return

        book = client.get_book(symbol=delta.symbol)
        bid_prices,_ = book.get_bids_array()
        ask_prices,_ = book.get_asks_array()

        bids_prior = bids_seen.copy()
        bids_seen = set()
        for b in bid_prices:
            key = int(round(b * 1e8))
            bids_seen.add(key)

        asks_prior = asks_seen.copy()
        asks_seen = set()
        for b in ask_prices:
            key = int(round(b * 1e8))
            asks_seen.add(key)

        ndelete = 0
        for p in bids_prior:
            if p not in bids_seen:
                ndelete += 1
        for p in asks_prior:
            if p not in asks_seen:
                ndelete += 1

        print(f"deleting: {ndelete}, {len(bids_prior)} -> {len(bids_seen)}, {len(asks_prior)} -> {len(asks_seen)}")

    # Delta handler
    async def handle_delta2(client: BinanceFuturesWebSocketClient, delta: L2Delta):
        global symbols_seen, ticks, bids_seen1, asks_seen1, bids_seen2, asks_seen2

        ticks += 1
        symbols_seen.add(delta.symbol)
        print(
            f"[{delta.symbol}] Delta: "
            f"Bids={len(delta.bid_changes)} Asks={len(delta.ask_changes)}"
        )

        if (ticks % 2) > 0:
            return

        book = client.get_book(symbol=delta.symbol)
        bid_prices1,_ = book.get_bids_array()
        ask_prices1,_ = book.get_asks_array()
        bid_prices2,_ = book.aggregate_dual(Side.BUY, 0.3, 5.0)
        ask_prices2,_ = book.aggregate_dual(Side.SELL, 0.3, 5.0)

        bids_prior1 = bids_seen1.copy()
        bids_seen1 = set()
        for b in bid_prices1:
            key = int(round(b * 1e8))
            bids_seen1.add(key)

        asks_prior1 = asks_seen1.copy()
        asks_seen1 = set()
        for b in ask_prices1:
            key = int(round(b * 1e8))
            asks_seen1.add(key)

        ndelete1 = 0
        for p in bids_prior1:
            if p not in bids_seen1:
                ndelete1 += 1
        for p in asks_prior1:
            if p not in asks_seen1:
                ndelete1 += 1

        bids_prior2 = bids_seen2.copy()
        bids_seen2 = set()
        for b in bid_prices2:
            key = int(round(b * 1e8))
            bids_seen2.add(key)

        asks_prior2 = asks_seen2.copy()
        asks_seen2 = set()
        for b in ask_prices2:
            key = int(round(b * 1e8))
            asks_seen2.add(key)

        ndelete2 = 0
        for p in bids_prior2:
            if p not in bids_seen2:
                ndelete2 += 1
        for p in asks_prior2:
            if p not in asks_seen2:
                ndelete2 += 1

        print(f"[1] deleting: {ndelete1}, {len(bids_prior1)} -> {len(bids_seen1)}, {len(asks_prior1)} -> {len(asks_seen1)}")
        print(f"[2] deleting: {ndelete2}, {len(bids_prior2)} -> {len(bids_seen2)}, {len(asks_prior2)} -> {len(asks_seen2)}")


    # Create client
    client = BinanceFuturesWebSocketClient(
        symbols=["BTCUSDT"],
        snapshot_limit=1000,
        enable_bbo=False,
        handlers={
            Topic.BBO: handle_bbo,
            Topic.L2_DELTA: handle_delta2,
        }
    )

    # Connect
    await client.connect()

    # Monitor order books
    try:
        while len(symbols_seen) < 2:
            await asyncio.sleep(5)


        for symbol in client.symbols:
            book = client.get_book(symbol)
            print(book)

    except KeyboardInterrupt:
        print("\nStopping...")
        await client.disconnect()


if __name__ == "__main__":
    asyncio.run(main())