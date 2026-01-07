"""
OrderBook Side Manager for Binance Replication

Manages a collection of orders on one side (bid or ask) with:
- Chunked order sizing (quarters by default)
- Automatic synchronization with Binance book
- Order state tracking
- Discrepancy calculation
"""

import numpy as np
from dataclasses import dataclass
from typing import Dict, List, Set, Tuple, Optional
from collections import defaultdict

from bulk.common import Side, OrderStatus, TimeInForce
from bulk.messages.account import OrderState
from bulk.data import FastOrderBook
from bulk.messages import L2Snapshot, OrderBookLevel


@dataclass
class OrderInfo:
    """Information about a single order at a price level"""
    order_id: str
    price: float
    size: float
    side: Side
    status: OrderStatus
    timestamp: int = 0

    @property
    def is_active(self) -> bool:
        """Check if order is still active (not terminal)"""
        return not self.status.is_terminal()


class PriceLevel:
    """
    Represents a single price level with multiple chunked orders

    Each level can have multiple orders sized in chunks to allow
    granular size adjustments as Binance book changes.
    """

    def __init__(self, price: float, side: Side, chunk_size: float):
        """
        Initialize price level

        Args:
            price: Price level
            side: Side.BUY or Side.SELL
            chunk_size: Base size for each order chunk
        """
        self.price = price
        self.side = side
        self.chunk_size = chunk_size
        self.orders: Dict[str, OrderInfo] = {}  # order_id -> OrderInfo

    @property
    def total_size(self) -> float:
        """Total size resting at this level (active orders only)"""
        return sum(
            order.size for order in self.orders.values()
            if order.is_active
        )

    @property
    def num_active_orders(self) -> int:
        """Number of active orders at this level"""
        return sum(1 for order in self.orders.values() if order.is_active)

    def add_order(self, order_id: str, size: float, timestamp: int = 0):
        """
        Add a new order to this level

        Args:
            order_id: Order ID
            size: Order size
            timestamp: Order timestamp
        """
        self.orders[order_id] = OrderInfo(
            order_id=order_id,
            price=self.price,
            size=size,
            side=self.side,
            status=OrderStatus.NONE,  # Will be updated when order is placed
            timestamp=timestamp
        )

    def update_order(self, order_state: OrderState):
        """
        Update order status from exchange feed

        Args:
            order_state: OrderState update from exchange
        """
        order_id = order_state.order_id
        if order_id in self.orders:
            order = self.orders[order_id]
            order.status = order_state.status
            order.size = order_state.size  # Update remaining size
            order.timestamp = order_state.timestamp

    def get_orders_to_cancel(self, target_size: float) -> List[str]:
        """
        Determine which orders to cancel to reach target size

        Strategy: Cancel largest orders first to minimize number of cancels

        Args:
            target_size: Target total size for this level

        Returns:
            List of order IDs to cancel
        """
        current_size = self.total_size
        if current_size <= target_size:
            return []

        # Calculate how much to reduce
        size_to_reduce = current_size - target_size

        # Get active orders sorted by size (largest first)
        active_orders = [
            order for order in self.orders.values()
            if order.is_active
        ]
        active_orders.sort(key=lambda x: x.size, reverse=True)

        # Select orders to cancel
        to_cancel = []
        reduced = 0.0
        for order in active_orders:
            if reduced >= size_to_reduce:
                break
            to_cancel.append(order.order_id)
            reduced += order.size

        return to_cancel

    def get_size_to_add(self, target_size: float) -> float:
        """
        Calculate how much size needs to be added to reach target

        Args:
            target_size: Target total size for this level

        Returns:
            Size that needs to be added (0 if no addition needed)
        """
        current_size = self.total_size
        return max(0.0, target_size - current_size)

    def cleanup_terminal_orders(self):
        """Remove terminal orders (filled/cancelled) to free memory"""
        terminal_oids = [
            oid for oid, order in self.orders.items()
            if order.status.is_terminal()
        ]
        for oid in terminal_oids:
            del self.orders[oid]

    def __repr__(self) -> str:
        return (
            f"PriceLevel(price={self.price:.2f}, "
            f"size={self.total_size:.4f}, "
            f"orders={self.num_active_orders})"
        )


class OrderStack:
    """
    Manages orders for one side of the book (bid or ask)

    Features:
    - Tracks orders at each price level
    - Sizes orders in chunks (quarters by default)
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
            side: Side,
            chunk_fraction: float = 0.25,  # 1/4 = 0.25 for quarters
            min_orders_per_level: int = 4,
            symbol: str = "BTC-USD",
            max_price_levels: int = 100,
    ):
        """
        Initialize order book side manager

        Args:
            side: Side.BUY or Side.SELL
            chunk_fraction: Fraction for sizing chunks (0.25 = quarters)
            min_orders_per_level: Minimum orders per price level
            symbol: Trading symbol
            max_price_levels: Maximum price levels to track
        """
        self.side = side
        self.chunk_fraction = chunk_fraction
        self.min_orders_per_level = min_orders_per_level
        self.symbol = symbol
        self.max_price_levels = max_price_levels

        # Price level tracking
        self.levels: Dict[float, PriceLevel] = {}  # price -> PriceLevel

        # Order tracking
        self.order_map: Dict[str, float] = {}  # order_id -> price

        # Statistics
        self.total_orders_placed = 0
        self.total_orders_cancelled = 0

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
            'total_orders_placed': self.total_orders_placed,
            'total_orders_cancelled': self.total_orders_cancelled,
        }

    # Primary interface method

    async def sync(
        self,
        ws_client,
        binance_prices: np.ndarray,
        binance_sizes: np.ndarray,
        tolerance: float = 0.1
    ) -> Tuple[List[str], List[str]]:
        """
        Sync with Binance book and execute order changes via WebSocket

        This is the PRIMARY interface method - call this in your main loop.
        Determines which orders to place/cancel, then executes via WebSocket.

        Args:
            ws_client: BulkWebSocketClient instance
            binance_prices: Array of price levels from Binance (sorted)
            binance_sizes: Array of sizes at each price level
            tolerance: Tolerance for size difference (fraction of chunk_size)

        Returns:
            Tuple of:
            - List of placed order IDs
            - List of cancelled order IDs

        Example:
            placed, cancelled = await bid_side.sync(
                ws_client, binance_prices, binance_sizes
            )
        """
        # Determine actions
        orders_to_place, orders_to_cancel = self.plan(
            binance_prices, binance_sizes, tolerance
        )

        # Execute actions
        placed_oids, cancelled_oids = await self._place_and_cancel(
            ws_client, orders_to_place, orders_to_cancel
        )

        return placed_oids, cancelled_oids

    # Planning method (determine actions without execution)

    def plan(
        self,
        binance_prices: np.ndarray,
        binance_sizes: np.ndarray,
        tolerance: float = 0.1
    ) -> Tuple[List[Tuple[float, float]], List[str]]:
        """
        Plan order changes needed to sync with Binance (without execution)

        Use this when you want to see what would happen before executing,
        or when implementing custom execution logic. For normal usage,
        just call sync() instead.

        Args:
            binance_prices: Array of price levels from Binance (sorted)
            binance_sizes: Array of sizes at each price level
            tolerance: Tolerance for size difference (fraction of chunk_size)

        Returns:
            Tuple of:
            - List of (price, size) tuples for all new orders to place
            - List of all order_ids to cancel

        Example:
            orders, cancels = bid_side.plan(binance_prices, binance_sizes)
            print(f"Would place {len(orders)} and cancel {len(cancels)}")
        """
        all_orders_to_place = []
        all_orders_to_cancel = []

        # Track which prices we've seen from Binance
        seen_prices = set()

        # Limit to max_price_levels
        n_levels = min(len(binance_prices), self.max_price_levels)

        # Sync each Binance level
        for i in range(n_levels):
            price = binance_prices[i]
            size = binance_sizes[i]
            seen_prices.add(price)

            orders_to_place, orders_to_cancel = self._sync_level(
                price, size, tolerance
            )
            all_orders_to_place.extend(orders_to_place)
            all_orders_to_cancel.extend(orders_to_cancel)

        # Cancel orders at levels no longer in Binance book
        for price in list(self.levels.keys()):
            if price not in seen_prices:
                level = self.levels[price]
                orders_to_cancel = [
                    oid for oid, order in level.orders.items()
                    if order.is_active
                ]
                all_orders_to_cancel.extend(orders_to_cancel)

        return all_orders_to_place, all_orders_to_cancel

    # State management

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

        # Track order location
        if order_id not in self.order_map:
            self.order_map[order_id] = price

        # Update level if it exists
        if price in self.levels:
            level = self.levels[price]
            level.update_order(order_state)

            # Cleanup if needed
            if order_state.status.is_terminal():
                level.cleanup_terminal_orders()

    def get_discrepancy(
        self,
        binance_prices: np.ndarray,
        binance_sizes: np.ndarray
    ) -> Dict[float, float]:
        """
        Calculate size discrepancy at each level vs Binance (for monitoring)

        Returns:
            Dict of price -> size_difference
            - Positive: we have more size than target
            - Negative: we have less size than target
        """
        discrepancy = {}

        for price, binance_size in zip(binance_prices, binance_sizes):
            target_size = self._calculate_target_size(binance_size)
            current_size = (
                self.levels[price].total_size
                if price in self.levels
                else 0.0
            )
            discrepancy[price] = current_size - target_size

        return discrepancy

    def get_total_discrepancy(
            self,
            binance_prices: np.ndarray,
            binance_sizes: np.ndarray
    ) -> float:
        """
        Calculate total absolute size discrepancy vs Binance (for monitoring)

        Returns:
            Total absolute size difference across all levels
        """
        discrepancies = self.get_discrepancy(binance_prices, binance_sizes)
        return sum(abs(diff) for diff in discrepancies.values())

    def cleanup(self):
        """Clean up terminal orders and empty levels"""
        # Cleanup each level
        for level in self.levels.values():
            level.cleanup_terminal_orders()

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

    def _register_pending_order(
            self,
            order_id: str,
            price: float,
            size: float,
            timestamp: int = 0
    ):
        """
        Register a pending order that was just placed (internal use)

        This is called automatically by place_and_cancel(). Only call this
        directly if you're placing orders through a different mechanism.

        Args:
            order_id: Order ID from exchange (or temporary client ID)
            price: Order price
            size: Order size
            timestamp: Order timestamp
        """
        if price not in self.levels:
            # Estimate chunk size based on order size
            chunk = size
            self.levels[price] = PriceLevel(price, self.side, chunk)

        level = self.levels[price]
        level.add_order(order_id, size, timestamp)
        self.order_map[order_id] = price
        self.total_orders_placed += 1


    async def _place_and_cancel(
            self,
            ws_client,
            orders_to_place: List[Tuple[float, float]],
            orders_to_cancel: List[str]
    ) -> Tuple[List[str], List[str]]:
        """
        Execute order placement and cancellation via WebSocket (internal)

        Called automatically by sync(). Only call directly if implementing
        custom execution logic.

        Args:
            ws_client: BulkWebSocketClient instance
            orders_to_place: List of (price, size) tuples
            orders_to_cancel: List of order IDs to cancel

        Returns:
            Tuple of:
            - List of placed order IDs (from exchange responses)
            - List of cancelled order IDs (from cancellation requests)
        """
        placed_oids = []
        cancelled_oids = []

        # Build list of actions for batch execution
        from bulk.messages.trade import LimitOrder, CancelOrder
        actions = []

        # Add placement orders
        for price, size in orders_to_place:
            order = LimitOrder(
                symbol=self.symbol,
                side=self.side,
                price=price,
                size=size,
                reduce_only=False,
                time_in_force=TimeInForce.GTC
            )
            actions.append(order)

        # Add cancellation orders
        for order_id in orders_to_cancel:
            cancel = CancelOrder(
                symbol=self.symbol,
                oid=order_id
            )
            actions.append(cancel)

        # Execute batch if there are actions
        if actions:
            try:
                responses = await ws_client.place_multi(actions)

                # Process responses
                for i, response in enumerate(responses):
                    if i < len(orders_to_place):
                        # This was a placement
                        price, size = orders_to_place[i]
                        if response.order_id:
                            # Order was placed successfully
                            self._register_pending_order(
                                response.order_id, price, size
                            )
                            placed_oids.append(response.order_id)
                    else:
                        # This was a cancellation
                        cancel_idx = i - len(orders_to_place)
                        order_id = orders_to_cancel[cancel_idx]
                        if response.status == OrderStatus.CANCELLED:
                            # Mark as cancelled in our tracking
                            if order_id in self.order_map:
                                price = self.order_map[order_id]
                                if price in self.levels:
                                    level = self.levels[price]
                                    if order_id in level.orders:
                                        level.orders[order_id].status = OrderStatus.CANCELLED
                            cancelled_oids.append(order_id)
                            self.total_orders_cancelled += 1

            except Exception as e:
                # Log error but don't crash
                import logging
                logger = logging.getLogger(__name__)
                logger.error(f"Error executing orders: {e}")

        return placed_oids, cancelled_oids

    def _calculate_chunk_size(self, target_size: float) -> float:
        """
        Calculate the chunk size for dividing target_size

        Args:
            target_size: Total size at the level from Binance

        Returns:
            Size of each chunk
        """
        return target_size * self.chunk_fraction

    def _calculate_target_size(self, binance_size: float) -> float:
        """
        Round Binance size to nearest chunk multiple

        Ensures target is at least min_orders_per_level chunks to maintain
        granular control over the level.

        Args:
            binance_size: Size from Binance orderbook

        Returns:
            Rounded target size as multiple of chunks
        """
        if binance_size == 0:
            return 0.0

        # Calculate chunk size
        chunk = binance_size * self.chunk_fraction

        # Round to nearest multiple of chunks, ensuring at least min_orders_per_level
        num_chunks = max(self.min_orders_per_level, round(binance_size / chunk))

        return num_chunks * chunk

    def _sync_level(
            self,
            price: float,
            binance_size: float,
            tolerance: float = 0.1
    ) -> Tuple[List[Tuple[float, float]], List[str]]:
        """
        Synchronize a single price level with Binance book

        Args:
            price: Price level
            binance_size: Size at this level on Binance
            tolerance: Tolerance for size difference (fraction of chunk_size)

        Returns:
            Tuple of:
            - List of (price, size) tuples for new orders to place
            - List of order_ids to cancel
        """
        # Calculate target size (rounded to chunk multiples)
        target_size = self._calculate_target_size(binance_size)

        # Get or create level
        if price not in self.levels:
            if target_size == 0:
                return [], []
            chunk = self._calculate_chunk_size(binance_size)
            self.levels[price] = PriceLevel(price, self.side, chunk)

        level = self.levels[price]
        current_size = level.total_size

        orders_to_place = []
        orders_to_cancel = []

        # Calculate size difference
        size_diff = target_size - current_size

        # Check if within tolerance (avoid unnecessary churn)
        tolerance_size = level.chunk_size * tolerance
        if abs(size_diff) < tolerance_size:
            return [], []

        if size_diff > 0:
            # Need to add orders
            size_to_add = level.get_size_to_add(target_size)
            chunk = level.chunk_size

            # Create orders in chunks (at least 1)
            num_orders = max(1, int(np.ceil(size_to_add / chunk)))
            order_size = size_to_add / num_orders

            for _ in range(num_orders):
                orders_to_place.append((price, order_size))

        elif size_diff < 0:
            # Need to cancel orders
            orders_to_cancel = level.get_orders_to_cancel(target_size)

        # If target is zero, cancel all orders at this level
        if target_size == 0:
            orders_to_cancel = [
                oid for oid, order in level.orders.items()
                if order.is_active
            ]

        return orders_to_place, orders_to_cancel

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


def example_usage():
    """Demonstrate OrderBookSide usage"""
    import time

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
        chunk_fraction=0.25,  # Quarter sizing
        min_orders_per_level=4,
        symbol="BTC-USD"
    )

    print(f"Initial: {bid_side}\n")

    # Get Binance bids
    binance_prices, binance_sizes = binance_book.get_bids_array(n=10)

    # Initial sync
    orders_to_place, orders_to_cancel = bid_side.plan(
        binance_prices, binance_sizes
    )

    print(f"Initial sync:")
    print(f"  Orders to place: {len(orders_to_place)}")
    print(f"  Orders to cancel: {len(orders_to_cancel)}")

    # Simulate placing orders (in real usage, execute_sync() does this automatically)
    # Here we manually call the private method for demonstration
    for i, (price, size) in enumerate(orders_to_place[:5]):
        order_id = f"order_{i}"
        bid_side._register_pending_order(order_id, price, size)  # Private method - normally done by execute_sync()
        print(f"  Placed: {order_id} - {size:.4f} @ {price:.2f}")

    print(f"\nAfter placing orders: {bid_side}")
    print(f"Level summary: {bid_side.get_level_summary()}")

    # Simulate order state updates (orders becoming resting)
    for i in range(5):
        order_id = f"order_{i}"
        price = bid_side.order_map[order_id]
        order_state = OrderState(
            timestamp=int(time.time() * 1e9),
            symbol="BTC-USD",
            order_id=order_id,
            status=OrderStatus.RESTING,
            side=Side.BUY,
            price=price,
            vwap=price,
            size=orders_to_place[i][1],
            size_done=0.0,
            size_orig=orders_to_place[i][1],
            is_maker=True
        )
        bid_side.update_order_state(order_state)

    print(f"\nAfter order updates: {bid_side}")

    # Check discrepancy
    discrepancy = bid_side.get_discrepancy(binance_prices[:5], binance_sizes[:5])
    print(f"\nDiscrepancy vs Binance:")
    for price, diff in list(discrepancy.items())[:5]:
        print(f"  {price:.2f}: {diff:+.4f}")

    print(f"\nTotal absolute discrepancy: {bid_side.get_total_discrepancy(binance_prices[:5], binance_sizes[:5]):.4f}")

    # Get stats
    stats = bid_side.get_stats()
    print(f"\nStatistics:")
    for key, value in stats.items():
        print(f"  {key}: {value}")


if __name__ == "__main__":
    example_usage()