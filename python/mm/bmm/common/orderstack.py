"""
OrderBook Side Manager for Binance Replication

Manages a collection of orders on one side (bid or ask) with:
- Chunked order sizing (quarters by default)
- Automatic synchronization with Binance book
- Order state tracking
- Discrepancy calculation
"""
import time

import base58
import numpy as np
from dataclasses import dataclass
from typing import Dict, List, Set, Tuple, Optional

from bulk.api import BulkWebSocketClient
from bulk.common import Side, OrderStatus, TimeInForce, TransactionSigner
from bulk.messages.account import OrderState
from bulk.data import FastOrderBook
from bulk.messages import L2Snapshot, OrderBookLevel, LimitOrder, CancelOrder

import logging
logger = logging.getLogger(__name__)


class PriceLevel:
    """
    Represents a single price level with multiple chunked orders

    Each level can have multiple orders sized in chunks to allow
    granular size adjustments as Binance book changes.
    """

    def __init__(self, symbol, price: float, side: Side, chunk_size: float, decimals: int):
        """
        Initialize price level

        Args:
            symbol: The symbol for this price level
            price: Price level
            side: Side.BUY or Side.SELL
            chunk_size: Base size for each order chunk
            decimals: Number of decimal digits
        """
        self.symbol = symbol
        self.price = price
        self.side = side
        self.chunk_size = chunk_size
        self.decimal_scale = 10.0**decimals

        self.pending_placed = 0
        self.pending_cancel = 0
        self.current_size = 0
        self.orders: Dict[str, OrderState] = {}

    @property
    def total_size(self) -> float:
        """Total size resting at this level (active orders only)"""
        return self.current_size

    @property
    def effective_size(self) -> float:
        """Effective size of this level, including in-flight"""
        return self.current_size + self.pending_placed - self.pending_cancel

    @property
    def num_active_orders(self) -> int:
        """Number of active orders at this level"""
        return len(self.orders)

    def inflight_place(self, order_state: OrderState):
        """
        Add an in-flight order

        Args:
            order_state: Order state to add
        """
        order_id = order_state.order_id
        logger.debug(f"Adding in-flight order {order_id}")
        self.pending_placed += order_state.size
        self.orders[order_id] = OrderState(**vars(order_state))
        self.orders[order_id].status = OrderStatus.NONE

    def inflight_cancel(self, tocancel: OrderState):
        """
        Add an in-flight cancel

        Args:
            order_state: Order to be cancelled
        """
        order_id = tocancel.order_id
        if order_id in self.orders:
            state = OrderState(**vars(self.orders[order_id]))
            state.timestamp = time.time_ns()
            state.status = OrderStatus.CANCEL_PENDING
            self.orders[order_id] = state
            self.pending_cancel += tocancel.size
        else:
            # order not found, so nothing to do
            pass

    def update_order(self, order_state: OrderState):
        """
        Update order status from exchange feed

        Args:
            order_state: OrderState update from exchange
        """
        order_id = order_state.order_id
        if order_id not in self.orders:
            self.orders[order_id] = order_state

        prior_state = self.orders[order_id]
        if prior_state.status == OrderStatus.NONE:
            self.pending_placed = max(self.pending_placed - order_state.size, 0.0)
            self.current_size = self.current_size + order_state.size
        elif prior_state.status == OrderStatus.CANCEL_PENDING:
            self.pending_cancel = max(self.pending_cancel - order_state.size, 0.0)

        if order_state.status.is_terminal():
            logger.debug(f"Order {order_id} is terminal, removing")
            self.current_size = max(self.current_size - order_state.size, 0.0)
            self.orders.pop(order_id, None)
        else:
            self.orders[order_id] = order_state

    def plan_resize(
        self,
        target_size: float,
        signer: TransactionSigner,
        tif: TimeInForce,
        tolerance: float = 0.1,
        max_orders: int = 5,
        nonce: int = 0,
    ) -> Tuple[List[LimitOrder], List[CancelOrder]]:
        """
        Synchronize a single price level with requested book

        Args:
            target_size: target size for this level
            tolerance: Tolerance for size difference (fraction of chunk_size)
            max_orders: maximum number of orders to request
            nonce: nonce for these orders

        Returns:
            Tuple of:
            - List of limit orders to add
            - List of cancel orders to add
        """
        # Calculate target size (rounded to chunk multiples)
        current_size = self.effective_size
        orders_to_place = []
        orders_to_cancel = []

        # Calculate size difference
        size_diff = target_size - current_size

        # Check if within tolerance (avoid unnecessary churn)
        tolerance_size = self.chunk_size * tolerance
        if abs(size_diff) < tolerance_size:
            return [], []

        if size_diff > 0:
            # Need to add orders
            size_to_add = self._get_size_to_add(target_size)
            chunk = self.chunk_size

            # Create orders in chunks (at least 1)
            num_orders = max(1, int(np.ceil(size_to_add / chunk)))
            if num_orders > max_orders:
                order_size = size_to_add / max_orders
                num_orders = max_orders
            else:
                order_size = self.chunk_size

            for i in range(num_orders):
                jitter = 10.0 * i / self.decimal_scale / 10.0
                order = LimitOrder(
                    symbol=self.symbol,
                    side=self.side,
                    price=self.price,
                    size=order_size + jitter,
                    reduce_only=False,
                    time_in_force=tif,
                    pubkey=signer.public_key,
                    nonce=nonce,
                )
                orders_to_place.append(order)

                # placeholder for in-flight orders
                state = order.to_state(OrderStatus.NONE)
                self.inflight_place(state)

        elif size_diff < 0 and target_size > 0:
            # Need to cancel orders
            for oid in self._get_orders_to_cancel(target_size):
                action = CancelOrder(
                    symbol=self.symbol,
                    order_id=oid,
                    side=self.side,
                    nonce=nonce
                )
                orders_to_cancel.append(action)

                # placeholder for in-flight cancel
                state = self.orders.get(oid, None)
                self.inflight_cancel(state)

        elif target_size == 0:
            # If target is zero, cancel all orders at this level
            orders_to_cancel.extend(self.plan_cancel())

        return orders_to_place, orders_to_cancel

    def plan_cancel(self) -> List[CancelOrder]:
        """
        Cancel all orders in level
        """
        actions = []
        for oid, order in self.orders.items():
            if order.status != OrderStatus.CANCEL_PENDING:
                action = CancelOrder(
                    symbol=self.symbol,
                    oid=oid,
                    side=self.side,
                )
                actions.append(action)

                # placeholder for in-flight cancel
                self.inflight_cancel(order)
        return actions

    def _get_orders_to_cancel(self, target_size: float) -> List[str]:
        """
        Determine which orders to cancel to reach target size

        Strategy: Cancel largest orders first to minimize number of cancels

        Args:
            target_size: Target total size for this level

        Returns:
            List of order IDs to cancel
        """
        current_size = self.effective_size
        if current_size <= target_size:
            return []

        # Calculate how much to reduce
        size_to_reduce = current_size - target_size

        # Get active orders sorted by size (largest first)
        active_orders = list(self.orders.values())
        active_orders.sort(key=lambda x: x.size, reverse=True)

        # Select orders to cancel
        to_cancel = []
        reduced = 0.0
        for order in active_orders:
            if reduced >= size_to_reduce:
                break
            if order.status != OrderStatus.CANCEL_PENDING:
                to_cancel.append(order.order_id)
                reduced += order.size

        return to_cancel

    def _get_size_to_add(self, target_size: float) -> float:
        """
        Calculate how much size needs to be added to reach target, taking into account
        possible in-flight orders

        Args:
            target_size: Target total size for this level

        Returns:
            Size that needs to be added (0 if no addition needed)
        """
        current_size = max(self.current_size, 0.0) + max(self.pending_placed, 0.0)
        return max(0.0, target_size - current_size)

    def __repr__(self) -> str:
        return (
            f"PriceLevel(price={self.price:.2f}, "
            f"size={self.total_size:.4f}, "
            f"effsize={self.effective_size:.4f}, "
            f"orders={self.num_active_orders})"
        )


class OrderStack:
    """
    Manages orders for one side of the book (bid or ask)

    Features:
    - Tracks orders at each price level
    - Sizes orders in chunks
    - Automatically determines which orders to add/cancel based on Binance book
    - Maintains minimum orders per level for granular control
    - Executes order placement/cancellation via WebSocket

    Public Methods (main interface):
        - sync(): Sync with Binance and execute via WebSocket
        - plan(): Determine order actions without execution
        - update_order_state(): Process order state updates
        - get_*(): Various getters for state/statistics
        - cleanup(): Remove terminal orders

    Usage:
        # Create bid side manager
        bid_side = OrderBookSide(Side.BUY, chunk_fraction=0.25, min_orders_per_level=4)

        # Get Binance book data
        binance_prices, binance_sizes = binance_book.get_bids_array(n=20)

        # Sync and execute in one call (PRIMARY METHOD)
        placed_orders, cancelled_orders = await bid_side.sync(
            ws_client, binance_prices, binance_sizes
        )

        # Or plan first, then execute separately
        orders_to_place, orders_to_cancel = bid_side.plan(
            binance_prices, binance_sizes
        )
        # ... custom logic ...
        placed, cancelled = await bid_side._place_and_cancel(
            ws_client, orders_to_place, orders_to_cancel
        )

        # Update from order feed (automatic via event handler)
        bid_side.update_order_state(order_state)
    """

    def __init__(
        self,
        symbol: str,
        side: Side,
        signer: TransactionSigner,
        chunk_size: float = 0.5,
        max_orders: int = 5,
        tif: TimeInForce = TimeInForce.GTC,
        max_price_levels: int = 1000,
        decimals: int = 8
    ):
        """
        Initialize order book side manager

        Args:
            symbol: Trading symbol
            side: Side.BUY or Side.SELL
            signer: Transaction signer
            chunk_size: order size
            max_orders: maximum number of orders to be added (a guideline)
            tif: TimeInForce
            max_price_levels: Maximum price levels to track
            decimals: Number of decimal digits when converting to int
        """
        self.symbol = symbol
        self.side = side
        self.signer = signer
        self.chunk_size = chunk_size
        self.max_orders = max_orders
        self.tif = tif
        self.max_price_levels = max_price_levels
        self.decimals = decimals
        self.decimal_scale = pow(10.0, decimals)

        # Price level tracking
        self.levels: Dict[int, PriceLevel] = {}  # price -> PriceLevel
        self.orders: Dict[str, OrderState] = {}


    # ==================== PUBLIC METHODS ====================

    # Primary getters (simple, fast queries)

    def get_total_size(self) -> float:
        """Get total size across all levels"""
        return sum(level.total_size for level in self.levels.values())

    def get_num_active_orders(self) -> int:
        """Get total number of active orders"""
        return sum(level.num_active_orders for level in self.levels.values())

    def get_num_levels(self) -> int:
        """Get number of active price levels"""
        return len([
            level for level in self.levels.values()
            if level.num_active_orders > 0
        ])

    def get_sorted_prices(self) -> np.ndarray:
        """
        Get sorted list of prices
        """
        iprices = list(self.levels.keys())
        if self.side == Side.BUY:
            iprices.sort(reverse=True)
            return np.array(iprices) / self.decimal_scale
        else:
            iprices.sort(reverse=False)
            return np.array(iprices) / self.decimal_scale

    def get_level_summary(self) -> Dict[float, Tuple[float, int]]:
        """
        Get summary of all levels

        Returns:
            Dict of price -> (total_size, num_active_orders)
        """
        return {
            price: (level.total_size, level.num_active_orders)
            for price, level in self.levels.items()
            if level.num_active_orders > 0
        }

    def get_stats(self) -> Dict:
        """
        Get statistics about this side

        Returns:
            Dict with statistics
        """
        return {
            'side': self.side.name,
            'num_levels': self.get_num_levels(),
            'num_active_orders': self.get_num_active_orders(),
            'total_size': self.get_total_size(),
        }

    # Primary interface method

    async def execute(
        self,
        ws_client: Optional[BulkWebSocketClient],
        new_prices: np.ndarray,
        new_sizes: np.ndarray,
        maxdepth: float = 2500.0,
        tolerance: float = 0.1,
        nonce: Optional[int] = None,
    ) -> Tuple[List[str], List[str], bool]:
        """
        Sync with reference book and execute order changes via WebSocket

        This is the PRIMARY interface method - call this in your main loop.
        Determines which orders to place/cancel, then executes via WebSocket.

        Args:
            ws_client: BulkWebSocketClient instance
            new_prices: Array of price levels (sorted)
            new_sizes: Array of sizes at each price level
            maxdepth: Max depth in bps to consider
            tolerance: Tolerance for size difference (fraction of chunk_size)
            nonce: Optional nonce

        Returns:
            new order IDs, cancelled order IDs

        Example:
            placed, cancelled = await bid_side.execute(
                ws_client, binance_prices, binance_sizes
            )
        """
        # Determine actions
        orders_to_place, orders_to_cancel, _, _ = self.plan(
            new_prices, new_sizes, maxdepth, tolerance, nonce=nonce
        )

        # Execute actions
        if ws_client:
            return await self._place_and_cancel(ws_client, orders_to_place, orders_to_cancel)
        else:
            return self._simulate_place_and_cancel(orders_to_place, orders_to_cancel)

    # Planning method (determine actions without execution)

    def plan(
        self,
        new_prices: np.ndarray,
        new_sizes: np.ndarray,
        maxdepth: float = 2500.0,
        tolerance: float = 0.1,
        nonce: Optional[int] = None,
    ) -> Tuple[List[LimitOrder], List[CancelOrder], List[int], List[int]]:
        """
        Plan order changes needed to sync with new sizes (without execution)

        Use this when you want to see what would happen before executing,
        or when implementing custom execution logic. For normal usage,
        just call execute() instead.

        Args:
            new_prices: Array of price levels from Binance (sorted)
            new_sizes: Array of sizes at each price level
            maxdepth: Max depth in bps to consider
            tolerance: Tolerance for size difference (fraction of chunk_size)
            nonce: Optional nonce

        Returns:
            Tuple of:
            - List of (price, size) tuples for all new orders to place
            - List of all order_ids to cancel

        Example:
            orders, cancels = bid_side.plan(binance_prices, binance_sizes)
            print(f"Would place {len(orders)} and cancel {len(cancels)}")
        """
        if not nonce:
            nonce = int(time.time_ns() / 1000)

        all_orders_to_place = []
        all_orders_to_cancel = []

        levels_added = []
        levels_deleted = []

        # Track which prices we've seen from Binance
        seen_prices = set()

        # Limit to max_price_levels
        n_levels = min(len(new_prices), self.max_price_levels)
        top = new_prices[0]

        prices_current = [int(1000.0 * x.price) for x in self.levels.values() if x.effective_size > 0.0]
        prices_next = [int(1000.0 * x) for x in new_prices[:n_levels+1]]

        # Sync each reference level
        for i in range(n_levels):
            price = new_prices[i]
            depth = np.abs(np.log(price / top)) * 1e4

            if depth >= maxdepth:
                break

            key = int(round(price * self.decimal_scale))
            size = new_sizes[i]
            seen_prices.add(key)
            target_size = round(size / self.chunk_size) * self.chunk_size

            if key not in self.levels:
                if target_size == 0:
                    continue
                levels_added.append(key * 1e-8)
                self.levels[key] = PriceLevel(self.symbol, price, self.side, self.chunk_size, self.decimals)
                level = self.levels[key]
            else:
                level = self.levels[key]

            orders_to_place, orders_to_cancel = level.plan_resize (
                target_size=size, tolerance=tolerance, nonce=nonce,
                max_orders=self.max_orders, tif=self.tif, signer=self.signer
            )
            all_orders_to_place.extend(orders_to_place)
            all_orders_to_cancel.extend(orders_to_cancel)

            for order in orders_to_place:
                self.orders[order.order_id()] = order.to_state(OrderStatus.NONE)

        # Cancel orders at levels no longer in Binance book
        for key in list(self.levels.keys()):
            if key not in seen_prices:
                level = self.levels[key]
                levels_deleted.append(key * 1e-8)
                all_orders_to_cancel.extend(level.plan_cancel())

        logger.info(f"{len(prices_current)} -> {len(prices_next)} added {len(all_orders_to_place)}, removed {len(levels_deleted)} / {len(all_orders_to_cancel)} {self.side.name}")
        return all_orders_to_place, all_orders_to_cancel, levels_added, levels_deleted

    # State management

    def terminated(self, order_id: str):
        """
        Notify that a given order has been terminated, in case was missed in state update
        """
        order = self.orders.get(order_id, None)
        if not order:
            return

        logger.debug(f"Order {order_id} has been terminated")
        order.status = OrderStatus.CANCELLED
        self.orders.pop(order_id, None)

        key = int(round(order.price * self.decimal_scale))
        level = self.levels.get(key, None)

        if not level:
            return

        level.update_order(order)
        if len(level.orders) == 0:
            self.levels.pop(key, None)


    def update_order_state(self, order_state: OrderState):
        """
        Update internal state based on order state update from exchange

        Args:
            order_state: OrderState from account feed
        """
        # Verify side matches
        if order_state.side != self.side:
            return

        order_id = order_state.order_id
        price = order_state.price
        key = int(round(price * self.decimal_scale))

        if not order_state.status.is_terminal():
            self.orders[order_id] = order_state
        else:
            self.orders.pop(order_id, None)

        # create level if does not exist
        if key not in self.levels:
            self.levels[key] = PriceLevel(self.symbol, price, self.side, self.chunk_size, self.decimals)

        # Update level
        level = self.levels[key]
        level.update_order(order_state)

        if len(level.orders) == 0:
            self.levels.pop(key, None)

    def get_discrepancy(
        self,
        prices: np.ndarray,
        sizes: np.ndarray
    ) -> Dict[float, float]:
        """
        Calculate size discrepancy at each level vs Binance (for monitoring)

        Returns:
            Dict of price -> size_difference
            - Positive: we have more size than target
            - Negative: we have less size than target
        """
        discrepancy = {}

        for price, size in zip(prices, sizes):
            key = int(round(price * self.decimal_scale))
            target_size = size
            current_size = (
                self.levels[key].total_size
                if key in self.levels
                else 0.0
            )
            discrepancy[price] = current_size - target_size

        return discrepancy

    async def cancel_all(self, ws_client: BulkWebSocketClient):
        """Cancel whole stack of orders"""
        actions = []
        for order in self.orders.values():
            order_id = order.order_id
            actions.append(CancelOrder(oid=order_id, symbol=self.symbol))

        return await ws_client.place_orders(actions)

    def get_total_discrepancy(
        self,
        prices: np.ndarray,
        sizes: np.ndarray
    ) -> float:
        """
        Calculate total absolute size discrepancy vs Binance (for monitoring)

        Returns:
            Total absolute size difference across all levels
        """
        discrepancies = self.get_discrepancy(prices, sizes)
        return sum(abs(diff) for diff in discrepancies.values())

    def cleanup(self):
        """Clean up terminal orders and empty levels"""
        # Remove empty levels
        empty_prices = [
            price for price, level in self.levels.items()
            if level.num_active_orders == 0
        ]
        for price in empty_prices:
            del self.levels[price]

    def __repr__(self) -> str:
        return (
            f"OrderBookSide(side={self.side.name}, "
            f"levels={self.get_num_levels()}, "
            f"orders={self.get_num_active_orders()}, "
            f"total_size={self.get_total_size():.4f})"
        )

    # ==================== PRIVATE METHODS ====================

    async def _place_and_cancel(
        self,
        ws_client,
        orders_to_place: List[LimitOrder],
        orders_to_cancel: List[CancelOrder],
    ) -> Tuple[List[str], List[str], bool]:
        """
        Execute order placement and cancellation via WebSocket (internal)

        Called automatically by sync(). Only call directly if implementing
        custom execution logic.

        Args:
            ws_client: BulkWebSocketClient instance
            orders_to_place: List of (price, size) tuples
            orders_to_cancel: List of order IDs to cancel

        Returns:
            added order_ids, cancelled order_ids, success
        """
        # Add placement orders
        actions = []
        actions.extend(orders_to_cancel)
        actions.extend(orders_to_place)

        if len(actions) == 0:
            return [], [], True

        # Execute batch if there are actions
        nonce = actions[0].nonce
        try:
            good = True
            responses = await ws_client.place_orders(actions, nonce=nonce)

            # Process responses
            added = []
            cancelled = []
            for i, response in enumerate(responses):
                if response.is_error():
                    good = False
                    logger.error(f"Error executing order: {actions[i]}: {response}")
                if response.status.is_placed():
                    added.append(response.order_id)
                elif response.status.is_cancelled():
                    cancelled.append(response.order_id)

            return added, cancelled, good
        except Exception as e:
            # Log error but don't crash
            logger.error(f"Error executing orders: {e}")
            return [], [], False


    def _simulate_place_and_cancel(
        self,
        orders_to_place: List[LimitOrder],
        orders_to_cancel: List[CancelOrder]
    ) -> Tuple[List[str], List[str], bool]:
        """
        Simulate execution of order placement and cancellation via WebSocket (internal)

        Args:
            orders_to_place: List of limit orders to place
            orders_to_cancel: List of orders to cancel

        Returns:
            true if all adjustments were successful
        """
        # Add placement orders
        tnow = time.time_ns()
        oid = tnow

        added_oid = []
        cancelled_oid = []
        for order in orders_to_place:
            order_id = base58.b58encode_int(oid).decode('utf-8')
            state = OrderState(
                timestamp = tnow,
                symbol = self.symbol,
                order_id = order_id,
                side = self.side,
                price = order.price,
                status = OrderStatus.RESTING,
                vwap = 0.0,
                size = order.size,
                size_done = 0.0,
                size_orig = order.size,
                is_maker = True
            )
            oid += 1
            added_oid.append(order_id)
            self.update_order_state(state)

        # Add cancellation orders
        for cancel in orders_to_cancel:
            old = self.orders.get(cancel.oid, None)
            size = old.size if old is not None else 0.0
            price = old.price if old is not None else 0.0
            state = OrderState(
                timestamp = tnow,
                symbol = self.symbol,
                order_id = cancel.oid,
                side = self.side,
                price = price,
                status = OrderStatus.CANCELLED,
                vwap = 0.0,
                size = size,
                size_done = 0.0,
                size_orig = size,
                is_maker = True
            )
            cancelled_oid.append(cancel.oid)
            self.update_order_state(state)

        return added_oid, cancelled_oid, True

    def __repr__(self) -> str:
        return (
            f"OrderBookSide(side={self.side.name}, "
            f"levels={self.get_num_levels()}, "
            f"orders={self.get_num_active_orders()}, "
            f"total_size={self.get_total_size():.4f})"
        )

    # ============================================================================
    # Example Usage
    # ============================================================================


async def example_usage():
    """Demonstrate OrderBookSide usage"""
    import time

    signer = TransactionSigner.generate_account()

    # test order state
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=98000.0,
        size=1.3,
        reduce_only=False,
        time_in_force=TimeInForce.GTC,
        nonce=int(time.time_ns() / 1000),
        pubkey=signer.public_key,
    )
    state = order.to_state(OrderStatus.NONE)
    print(state)

    # Create a synthetic Binance book
    bid_levels = [
        OrderBookLevel(price=100000.0 - i * 10, size=np.random.uniform(0.5, 2.0))
        for i in range(20)
    ]
    ask_levels = [
        OrderBookLevel(price=100000.0 + i * 10, size=np.random.uniform(0.5, 2.0))
        for i in range(20)
    ]

    binance_snapshot = L2Snapshot(
        timestamp=int(time.time() * 1000),
        symbol="BTC-USD",
        bids=bid_levels,
        asks=ask_levels
    )

    binance_book = FastOrderBook("BTC-USD")
    binance_book.from_snapshot(binance_snapshot)

    # Create bid side manager
    bid_side = OrderStack(
        side=Side.BUY,
        signer=signer,
        chunk_size=0.25,  # Quarter sizing
        max_orders=5,
        symbol="BTC-USD"
    )

    print(f"Initial: {bid_side}\n")

    # Get Binance bids
    binance_prices, binance_sizes = binance_book.get_bids_array(n=10)

    # Initial sync
    await bid_side.execute(
        ws_client=None,
        new_prices = binance_prices,
        new_sizes = binance_sizes,
        maxdepth=1000.0
    )

    # Check discrepancy
    discrepancy = bid_side.get_discrepancy(binance_prices[:5], binance_sizes[:5])
    print(f"\nDiscrepancy vs Binance:")
    for price, diff in list(discrepancy.items())[:5]:
        print(f"  {price:.2f}: {diff:+.4f}")

    print(f"\nTotal absolute discrepancy: {bid_side.get_total_discrepancy(binance_prices[:5], binance_sizes[:5]):.4f}")

    prices = bid_side.get_sorted_prices()
    print(f"\nPrices: {prices}")

    # Get stats
    stats = bid_side.get_stats()
    print(f"\nStatistics:")
    for key, value in stats.items():
        print(f"  {key}: {value}")

    # do some reductions
    binance_prices2 = binance_prices.copy()[1:]
    binance_sizes2 = binance_sizes.copy()[1:] / 2.0
    await bid_side.execute(
        ws_client=None,
        new_prices = binance_prices2,
        new_sizes = binance_sizes2,
        maxdepth=1000.0
    )

    # Check discrepancy
    discrepancy = bid_side.get_discrepancy(binance_prices2[:5], binance_sizes2[:5])
    print(f"\nDiscrepancy vs Binance 2nd update:")
    for price, diff in list(discrepancy.items())[:5]:
        print(f"  {price:.2f}: {diff:+.4f}")

    stats = bid_side.get_stats()
    print(f"\nStatistics:")
    for key, value in stats.items():
        print(f"  {key}: {value}")

if __name__ == "__main__":
    import asyncio
    asyncio.run(example_usage())