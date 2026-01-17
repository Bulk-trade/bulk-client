"""
Simulated Bulk Labs WebSocket Trading Client
Mimics BulkWebSocketClient behavior for backtesting and simulation
"""

import asyncio
import logging
import time
from typing import Dict, List, Optional, Callable, Set, Union
from dataclasses import dataclass, replace
from enum import Enum

import numpy as np

from bulk.common import Side, TimeInForce, OrderStatus
from bulk.common.inventory import Inventory, Pnl
from bulk.common.signer import TransactionSigner
from bulk.messages import SubscriptionRequest
from bulk.messages.account import AccountSnapshot, Margin, \
    LeverageSetting, MarginUpdate, PositionUpdate, OrderState
from bulk.messages.md import Ticker, Trade, L2Snapshot, L2Delta, Candle, OrderBookLevel
from bulk.messages.trade import OrderResponse, Fill, CancelOrder, LimitOrder, CancelAll, MarketOrder
from bulk.data import OrderBook, FastOrderBook
from bulk.common import Topic


@dataclass
class SimulatedOrder:
    """Internal order representation for simulation"""
    order: Union[LimitOrder, MarketOrder]
    order_id: str
    state: OrderState
    time_placed: float


class SimulatedWebSocketClient:
    """
    Simulated WebSocket client that mimics BulkWebSocketClient behavior

    Features:
    - Configurable latency for order responses and fills
    - Order matching against internal orderbook
    - Automatic state transitions (placed -> resting -> filled)
    - Fill simulation based on orderbook crossing
    - Full event emission matching real client

    Args:
        symbols: List of symbols to track
        signer: TransactionSigner for order signing
        inventory: Inventory instance for position tracking
        orderbook: External orderbook source (FastOrderBook or OrderBook)
        latency: Latency for order responses (seconds)
        logger: Logger instance
        handlers: Event handlers keyed by Topic
    """

    def __init__(
        self,
        symbols: List[str] = ["BTC-USD", "ETH-USD", "SOL-USD"],
        signer: Optional[TransactionSigner] = None,
        inventory: Optional[Inventory] = None,
        orderbook: Optional[Union[OrderBook, FastOrderBook]] = None,
        latency: float = 0.050,  # 50ms
        logger: Optional[logging.Logger] = None,
        handlers: Optional[Dict[Topic, Callable]] = None,
        debug: bool = False,
    ):
        self.symbols = symbols
        self.signer = signer
        self.inventory = inventory or Inventory()
        self.logger = logger or logging.getLogger(__name__)
        self.debug = debug

        # Latency configuration
        self.response_latency = latency

        # External orderbook (for matching)
        self.external_orderbook = orderbook

        # Event handlers
        self.handlers: Dict[Topic, List[Callable]] = {}
        for etype, handler in (handlers.items() if handlers else []):
            self.handlers[etype] = [handler]

        # Simulated state
        self.open_orders: Dict[str, SimulatedOrder] = {}
        self.prices: Dict[str, float] = {k: 100.0 for k in self.symbols}
        self.tickers: Dict[str, Ticker] = {}

        # Account state
        self.margin = Margin(
            total_balance=10000.0,
            available_balance=10000.0,
            margin_used=0.0,
            notional=0.0,
            realized_pnl=0.0,
            unrealized_pnl=0.0,
            fees=0.0,
            funding=0.0
        )
        self.leverage_settings: Dict[str, LeverageSetting] = {}

        # Order ID generation
        self.order_counter = 0

        # Connection state
        self._connected = False

        # Background tasks
        self._tasks: List[asyncio.Task] = []

    # ==================== CONNECTION MANAGEMENT ====================

    async def connect(self) -> bool:
        """Simulate connection"""
        self.logger.info("Simulated connection established")
        self._connected = True

        # Emit initial account snapshot
        await self._emit_account_snapshot()

        return True

    async def disconnect(self):
        """Disconnect and cancel all tasks"""
        self.logger.info("Simulated disconnection")
        self._connected = False

        # Cancel all background tasks
        for task in self._tasks:
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass
        self._tasks.clear()

    @property
    def is_connected(self) -> bool:
        """Check if connected"""
        return self._connected

    # ==================== SUBSCRIPTIONS (No-ops in simulation) ====================

    async def subscribe_ticker(self, symbol: str):
        """No-op in simulation"""
        pass

    async def subscribe_trades(self, symbols: List[str]):
        """No-op in simulation"""
        pass

    async def subscribe_orderbook_snapshot(self, symbol: str, nlevels: Optional[int] = None):
        """No-op in simulation"""
        pass

    async def subscribe_orderbook_delta(self, symbol: str):
        """No-op in simulation"""
        pass

    async def subscribe_account(self, user_pubkey: str):
        """No-op in simulation"""
        pass

    async def subscribe_candles(self, symbol: str, interval: str):
        """No-op in simulation"""
        pass

    # ==================== ORDERS ====================

    async def place_orders(
        self,
        actions: List[Union[LimitOrder, MarketOrder, CancelAll, CancelOrder]],
        timeout: Optional[float] = None,
    ) -> List[OrderResponse]:
        """
        Place multiple orders and/or cancels with simulated latency

        All actions in a batch share a single round-trip latency,
        matching real exchange behavior.

        Args:
            actions: List of order/cancel actions
            timeout: Not used in simulation

        Returns:
            List of OrderResponse objects
        """
        if not self.signer:
            raise RuntimeError("Signer not configured")
        if not self._connected:
            raise RuntimeError("Not connected")

        # Simulate single round-trip latency for entire batch
        latency = np.random.exponential(scale=self.response_latency)
        await asyncio.sleep(latency)

        responses = []

        for action in actions:
            if isinstance(action, CancelOrder):
                response = await self._handle_cancel_order_sync(action)
                responses.append(response)
            elif isinstance(action, CancelAll):
                cancel_responses = await self._handle_cancel_all_sync(action)
                responses.extend(cancel_responses)
            elif isinstance(action, (LimitOrder, MarketOrder)):
                response = await self._handle_place_order_sync(action)
                responses.append(response)

        return responses

    async def place_limit_order(
        self,
        symbol: str,
        side: Side,
        price: float,
        size: float,
        reduce_only: bool = False,
        time_in_force: TimeInForce = TimeInForce.GTC,
        timeout: Optional[float] = None,
    ) -> OrderResponse:
        """Place a limit order"""
        order = LimitOrder(
            symbol=symbol,
            side=side,
            price=price,
            size=size,
            reduce_only=reduce_only,
            time_in_force=time_in_force
        )

        responses = await self.place_orders([order], timeout)
        return responses[0]

    async def place_market_order(
        self,
        symbol: str,
        side: Side,
        size: float,
        reduce_only: bool = False,
        timeout: Optional[float] = None,
    ) -> OrderResponse:
        """Place a market order"""
        order = MarketOrder(
            symbol=symbol,
            side=side,
            size=size,
            reduce_only=reduce_only
        )

        responses = await self.place_orders([order], timeout)
        return responses[0]

    async def cancel_order(
        self,
        symbol: str,
        order_id: str,
        timeout: Optional[float] = None,
    ) -> OrderResponse:
        """Cancel an order"""
        cancel = CancelOrder(symbol=symbol, oid=order_id)
        responses = await self.place_orders([cancel], timeout)
        return responses[0]

    async def cancel_all(
        self,
        symbols: Optional[List[str]] = None,
        timeout: Optional[float] = None,
    ) -> int:
        """Cancel all orders"""
        cancel = CancelAll(symbols=symbols or self.symbols)
        responses = await self.place_orders([cancel], timeout)
        return len(responses)

    # ==================== INTERNAL ORDER HANDLING ====================

    async def _handle_place_order_sync(
        self,
        order: Union[LimitOrder, MarketOrder]
    ) -> OrderResponse:
        """Handle order placement without latency (called from place_orders)"""

        # Generate order ID using bulk-keychain
        if isinstance(order, LimitOrder):
            order_id, _ = self.signer.compute_limit_order_id(
                symbol=order.symbol,
                side=order.side,
                price=order.price,
                size=order.size,
                reduce_only=order.reduce_only,
                time_in_force=order.time_in_force,
            )
        else:
            order_id, _ = self.signer.compute_market_order_id(
                symbol=order.symbol,
                side=order.side,
                size=order.size,
                reduce_only=order.reduce_only,
            )

        # Create initial order state
        state = OrderState(
            timestamp=time.time_ns(),
            symbol=order.symbol,
            order_id=order_id,
            status=OrderStatus.RESTING,  # Initial status
            side=order.side,
            price=order.price if isinstance(order, LimitOrder) else 0.0,
            vwap=0.0,
            size=order.size,
            size_done=0.0,
            size_orig=order.size,
            is_maker=isinstance(order, LimitOrder)
        )

        # Store order
        sim_order = SimulatedOrder(
            order=order,
            order_id=order_id,
            state=state,
            time_placed=time.time()
        )
        self.open_orders[order_id] = sim_order

        # Create response
        response = OrderResponse(
            order_id=order_id,
            status=OrderStatus.RESTING,
            message=None,
            meta={"oid": order_id}
        )

        # Schedule order processing in background
        task = asyncio.create_task(self._process_order(sim_order))
        self._tasks.append(task)
        return response

    async def _process_order(self, sim_order: SimulatedOrder):
        """
        Process an order: emit RESTING state, then check for fills

        This simulates the order lifecycle:
        1. Immediate response (already done in _handle_place_order)
        2. RESTING status update
        3. Fill if order is aggressive (crosses the book)
        """
        try:
            # Emit RESTING order state
            await self._emit_event(Topic.ORDER, sim_order.state)

            self.logger.info(
                f"Order RESTING: {sim_order.order.symbol} "
                f"{sim_order.order.side} {sim_order.order.size} @ "
                f"{sim_order.order.price if isinstance(sim_order.order, LimitOrder) else 'MARKET'}"
            )

            # Check if order should fill immediately
            should_fill, fill_price = self._check_immediate_fill(sim_order)

            if should_fill:
                await self._fill_order(sim_order, fill_price)

        except asyncio.CancelledError:
            pass
        except Exception as e:
            self.logger.error(f"Error processing order: {e}", exc_info=True)

    def _check_immediate_fill(
        self,
        sim_order: SimulatedOrder
    ) -> tuple[bool, float]:
        """
        Check if order should fill immediately

        Returns:
            (should_fill, fill_price)
        """
        if not self.external_orderbook:
            # Without orderbook, only market orders fill immediately
            if isinstance(sim_order.order, MarketOrder):
                # Use last price or mid price as fill price
                mid_price = self.prices.get(sim_order.order.symbol, 100000.0)
                return True, mid_price
            return False, 0.0

        order = sim_order.order

        # Market orders always fill
        if isinstance(order, MarketOrder):
            # Get fill price from orderbook
            if order.side == Side.BUY:
                # Buying crosses asks
                best_ask = self.external_orderbook.get_best_ask()
                fill_price = best_ask if best_ask else self.prices.get(order.symbol, 100000.0)
            else:
                # Selling crosses bids
                best_bid = self.external_orderbook.get_best_bid()
                fill_price = best_bid if best_bid else self.prices.get(order.symbol, 100000.0)

            return True, fill_price

        # Limit orders fill if they cross the book
        if isinstance(order, LimitOrder):
            if order.side == Side.BUY:
                # Buy order crosses if price >= best ask
                best_ask = self.external_orderbook.get_best_ask()
                if best_ask and order.price >= best_ask:
                    return True, min(order.price, best_ask)
            else:
                # Sell order crosses if price <= best bid
                best_bid = self.external_orderbook.get_best_bid()
                if best_bid and order.price <= best_bid:
                    return True, max(order.price, best_bid)

        return False, 0.0

    async def _fill_order(self, sim_order: SimulatedOrder, fill_price: float):
        """Execute a fill for an order"""
        order = sim_order.order

        # Create fill
        fill = Fill(
            symbol=order.symbol,
            order_id=sim_order.order_id,
            client_id=None,
            price=fill_price,
            size=order.size,
            side=order.side,
            timestamp=time.time_ns(),
            is_maker=False  # Aggressive orders are takers
        )

        # Update inventory
        side_value = 1 if order.side == Side.BUY else -1
        self.inventory.traded(order.symbol, side_value, order.size, fill_price)

        # Update order state to FILLED
        sim_order.state = replace(
            sim_order.state,
            status=OrderStatus.FILLED,
            size_done=order.size,
            vwap=fill_price
        )

        # Remove from open orders
        self.open_orders.pop(sim_order.order_id, None)

        # Emit events
        await self._emit_event(Topic.FILL, fill)
        await self._emit_event(Topic.ORDER, sim_order.state)

        self.logger.info(
            f"Order FILLED: {order.symbol} "
            f"{order.side} {order.size} @ {fill_price:.2f}"
        )

        # Update position
        await self._emit_position_update(order.symbol)

    async def _handle_cancel_order_sync(self, cancel: CancelOrder) -> OrderResponse:
        """Handle order cancellation without latency (called from place_orders)"""

        # Check if order exists
        if cancel.oid not in self.open_orders:
            return OrderResponse(
                order_id=cancel.oid,
                status=OrderStatus.ERROR,
                message="Order not found",
                meta={}
            )

        sim_order = self.open_orders.pop(cancel.oid)

        # Update state to CANCELLED
        sim_order.state = replace(
            sim_order.state,
            status=OrderStatus.CANCELLED
        )

        # Emit cancelled state
        await self._emit_event(Topic.ORDER, sim_order.state)

        self.logger.info(f"Order CANCELLED: {cancel.oid}")

        return OrderResponse(
            order_id=cancel.oid,
            status=OrderStatus.CANCELLED,
            message=None,
            meta={"oid": cancel.oid}
        )

    async def _handle_cancel_all_sync(self, cancel_all: CancelAll) -> List[OrderResponse]:
        """Handle cancel all orders without latency (called from place_orders)"""

        symbols_to_cancel = set(cancel_all.symbols)
        responses = []

        # Cancel all matching orders
        orders_to_cancel = [
            (oid, order) for oid, order in self.open_orders.items()
            if order.order.symbol in symbols_to_cancel
        ]

        for oid, sim_order in orders_to_cancel:
            self.open_orders.pop(oid)

            # Update state to CANCELLED
            sim_order.state = replace(
                sim_order.state,
                status=OrderStatus.CANCELLED
            )

            # Emit cancelled state
            await self._emit_event(Topic.ORDER, sim_order.state)

            responses.append(OrderResponse(
                order_id=oid,
                status=OrderStatus.CANCELLED,
                message=None,
                meta={"oid": oid}
            ))

        self.logger.info(f"Cancelled {len(responses)} orders")

        return responses

    # ==================== EVENT HANDLERS ====================

    def on(self, event_type: Topic, handler: Callable):
        """Register an event handler"""
        if event_type not in self.handlers:
            self.handlers[event_type] = []
        self.handlers[event_type].append(handler)

    def off(self, event_type: Topic, handler: Callable):
        """Remove an event handler"""
        if event_type in self.handlers and handler in self.handlers[event_type]:
            self.handlers[event_type].remove(handler)

    async def _emit_event(self, event_type: Topic, event_data):
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

    # ==================== STATE UPDATES ====================

    async def _emit_account_snapshot(self):
        """Emit initial account snapshot"""
        positions = []
        for symbol in self.symbols:
            pos = self.inventory.position_for(symbol)
            if pos.quantity != 0:
                positions.append(pos)

        snapshot = AccountSnapshot(
            margin=self.margin,
            positions=[],
            open_orders=list(self.open_orders.values()),
            leverage_settings=list(self.leverage_settings.values())
        )

        await self._emit_event(Topic.ACCOUNT, snapshot)

    async def _emit_position_update(self, symbol: str):
        """Emit position update for a symbol"""
        pos = self.inventory.position_for(symbol)

        pos_update = PositionUpdate(
            symbol=symbol,
            size=pos.quantity,
            price=pos.vwap,
            realized_pnl=pos.realized_pnl,
            unrealized_pnl=pos.unrealized_pnl,
            leverage=1.0,
            liquidation_price=0.0,
            fair_price=pos.fair_price,
            notional=abs(pos.quantity * pos.fair_price),
            fees=0.0,
            funding=0.0,
            maintenance_margin=pos.mm,
            lambda_=pos.mmr,
            risk_allocation=pos.margin
        )

        await self._emit_event(Topic.POSITION, pos_update)

    # ==================== UTILITIES ====================

    def get_orders(self, symbol: Optional[str] = None) -> List[OrderState]:
        """Get open orders, optionally filtered by symbol"""
        orders = [sim.state for sim in self.open_orders.values()]
        if symbol:
            orders = [o for o in orders if o.symbol == symbol]
        return orders

    def get_pnl(self) -> Pnl:
        """Calculate P&L using Inventory class"""
        return self.inventory.pv(self.prices)

    def update_price(self, symbol: str, price: float):
        """Update market price for a symbol (for PnL calculation)"""
        self.prices[symbol] = price

        # Update fair price in positions
        pos = self.inventory.position_for(symbol)
        pos.fair_price = price