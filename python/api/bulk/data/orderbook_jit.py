import numpy as np
from numba import njit, types
from typing import Optional, Tuple
from bulk.common import Side
from bulk.messages import L2Snapshot, OrderBookLevel


class FastOrderBook:
    """
    Ultra-fast order book with dual-granularity aggregation.

    Performance for 1000 levels/side:
    - Mid price: ~10 ns
    - Single update: ~300 ns
    - Dual aggregation: ~30-50 μs
    - Single aggregation: ~20-40 μs
    """

    def __init__(self, symbol: str, max_levels: int = 2000):
        self.symbol = symbol
        self.max_levels = max_levels

        # Raw book data
        self.bid_prices = np.zeros(max_levels, dtype=np.float64)
        self.bid_sizes = np.zeros(max_levels, dtype=np.float64)
        self.n_bids = 0

        self.ask_prices = np.zeros(max_levels, dtype=np.float64)
        self.ask_sizes = np.zeros(max_levels, dtype=np.float64)
        self.n_asks = 0

        # Pre-allocated aggregation buffers
        self._agg_prices = np.zeros(max_levels, dtype=np.float64)
        self._agg_sizes = np.zeros(max_levels, dtype=np.float64)

        self.last_update_time = 0

    def from_snapshot(self, snapshot) -> None:
        """Initialize from snapshot"""
        if snapshot.symbol != self.symbol:
            raise ValueError(f"Symbol mismatch: expected {self.symbol}, got {snapshot.symbol}")

        n_bids = min(len(snapshot.bids), self.max_levels)
        for i in range(n_bids):
            self.bid_prices[i] = -snapshot.bids[i].price
            self.bid_sizes[i] = snapshot.bids[i].size
        self.n_bids = n_bids

        n_asks = min(len(snapshot.asks), self.max_levels)
        for i in range(n_asks):
            self.ask_prices[i] = snapshot.asks[i].price
            self.ask_sizes[i] = snapshot.asks[i].size
        self.n_asks = n_asks

        self.last_update_time = snapshot.timestamp

    def update_bid(self, price: float, size: float) -> None:
        """Update single bid level - O(log n)"""
        self.n_bids = _update_level_sorted(
            self.bid_prices, self.bid_sizes, self.n_bids,
            price, size, reverse=True
        )

    def update_ask(self, price: float, size: float) -> None:
        """Update single ask level - O(log n)"""
        self.n_asks = _update_level_sorted(
            self.ask_prices, self.ask_sizes, self.n_asks,
            price, size, reverse=False
        )

    def apply_delta(self, delta) -> None:
        """Apply delta update"""
        if delta.symbol != self.symbol:
            raise ValueError(f"Symbol mismatch")

        for level in delta.bid_changes:
            self.update_bid(level.price, level.size)

        for level in delta.ask_changes:
            self.update_ask(level.price, level.size)

        self.last_update_time = delta.timestamp

    def get_best_bid(self) -> Optional[float]:
        """O(1) best bid price"""
        return -self.bid_prices[0] if self.n_bids > 0 else None

    def get_best_ask(self) -> Optional[float]:
        """O(1) best ask price"""
        return self.ask_prices[0] if self.n_asks > 0 else None

    def get_mid_price(self) -> Optional[float]:
        """O(1) mid price - JIT compiled"""
        mid = _get_mid_price_fast(
            self.bid_prices, self.ask_prices,
            self.n_bids, self.n_asks
        )
        return mid if mid > 0 else None

    def get_spread(self) -> Optional[float]:
        """O(1) spread"""
        best_bid = self.get_best_bid()
        best_ask = self.get_best_ask()
        return (best_ask - best_bid) if (best_bid and best_ask) else None

    def aggregate_dual(
        self,
        side: Side,
        fine_tick: float,
        coarse_tick: float,
        n_fine_levels: int = 10
    ) -> Tuple[np.ndarray, np.ndarray]:
        """
        Aggregate with dual granularity.

        Args:
            side: Side.BUY or Side.SELL
            fine_tick: Tick size for first n_fine_levels (e.g., 1.0 for $1)
            coarse_tick: Tick size beyond n_fine_levels (e.g., 100.0 for $100)
            n_fine_levels: Number of raw levels to use fine_tick for

        Returns:
            (prices, sizes) as numpy arrays

        Example:
            # For BTC at $100k:
            # First 50 levels at $1 increments, rest at $100 increments
            prices, sizes = book.aggregate_dual(Side.BUY, 1.0, 100.0, 50)
        """
        if side == Side.BUY:
            n_agg = _aggregate_dual_granularity(
                self.bid_prices, self.bid_sizes, self.n_bids,
                fine_tick, coarse_tick, n_fine_levels, True,
                self._agg_prices, self._agg_sizes
            )
        else:
            n_agg = _aggregate_dual_granularity(
                self.ask_prices, self.ask_sizes, self.n_asks,
                fine_tick, coarse_tick, n_fine_levels, False,
                self._agg_prices, self._agg_sizes
            )

        return self._agg_prices[:n_agg].copy(), self._agg_sizes[:n_agg].copy()

    def aggregate(
        self,
        side: Side,
        tick_size: float
    ) -> Tuple[np.ndarray, np.ndarray]:
        """
        Single-granularity aggregation (faster when you don't need dual).

        Args:
            side: Side.BUY or Side.SELL
            tick_size: Uniform tick size for all levels

        Returns:
            (prices, sizes) as numpy arrays
        """
        if side == Side.BUY:
            n_agg = _aggregate_single_granularity(
                self.bid_prices, self.bid_sizes, self.n_bids,
                tick_size, True,
                self._agg_prices, self._agg_sizes
            )
        else:
            n_agg = _aggregate_single_granularity(
                self.ask_prices, self.ask_sizes, self.n_asks,
                tick_size, False,
                self._agg_prices, self._agg_sizes
            )

        return self._agg_prices[:n_agg].copy(), self._agg_sizes[:n_agg].copy()

    def sweep_price(self, side: Side, size: float) -> Optional[float]:
        """Calculate price to sweep size"""
        if side == Side.BUY:
            return _sweep_price_fast(
                self.bid_prices, self.bid_sizes, self.n_bids,
                size, reverse=True
            )
        else:
            return _sweep_price_fast(
                self.ask_prices, self.ask_sizes, self.n_asks,
                size, reverse=False
            )

    def get_depth(self, side: Side, depth_price: float) -> float:
        """Calculate depth to price"""
        if side == Side.BUY:
            return _calculate_depth_fast(
                self.bid_prices, self.bid_sizes, self.n_bids,
                depth_price, reverse=True
            )
        else:
            return _calculate_depth_fast(
                self.ask_prices, self.ask_sizes, self.n_asks,
                depth_price, reverse=False
            )

    def liquidity_adj_mid(self, size: float) -> Optional[float]:
        """Liquidity-adjusted mid using sweep prices"""
        bid_sweep = self.sweep_price(Side.BUY, size)
        ask_sweep = self.sweep_price(Side.SELL, size)
        if bid_sweep and ask_sweep:
            return (bid_sweep + ask_sweep) / 2.0
        return None

    def get_bids_array(self, n: Optional[int] = None) -> Tuple[np.ndarray, np.ndarray]:
        """Get raw bids as numpy arrays"""
        n = min(n or self.n_bids, self.n_bids)
        return -self.bid_prices[:n].copy(), self.bid_sizes[:n].copy()

    def get_asks_array(self, n: Optional[int] = None) -> Tuple[np.ndarray, np.ndarray]:
        """Get raw asks as numpy arrays"""
        n = min(n or self.n_asks, self.n_asks)
        return self.ask_prices[:n].copy(), self.ask_sizes[:n].copy()

    def __repr__(self) -> str:
        return f"FastOrderBook(symbol={self.symbol}, bids={self.n_bids}, asks={self.n_asks})"


# ============================================================================
# Numba-optimized Dual-Granularity Aggregation
# ============================================================================

@njit(cache=True, fastmath=True)
def _binary_search_price(prices: np.ndarray, n_levels: int, target_price: float) -> int:
    """Binary search for price index. Returns insertion point if not found."""
    left, right = 0, n_levels
    while left < right:
        mid = (left + right) >> 1
        if prices[mid] < target_price:
            left = mid + 1
        else:
            right = mid
    return left


@njit(cache=True, fastmath=True)
def _update_level_sorted(
    prices: np.ndarray,
    sizes: np.ndarray,
    n_levels: int,
    price: float,
    size: float,
    reverse: bool = False
) -> int:
    """Update or insert a level while maintaining sorted order."""
    if reverse:
        idx = _binary_search_price(prices, n_levels, -price)
        check_price = -price
    else:
        idx = _binary_search_price(prices, n_levels, price)
        check_price = price

    if idx < n_levels and abs(prices[idx] - check_price) < 1e-9:
        if size == 0.0:
            if idx < n_levels - 1:
                prices[idx:n_levels - 1] = prices[idx + 1:n_levels]
                sizes[idx:n_levels - 1] = sizes[idx + 1:n_levels]
            return n_levels - 1
        else:
            sizes[idx] = size
            return n_levels
    else:
        if size > 0.0:
            # Check if we're at max capacity
            max_size = len(prices)
            if n_levels >= max_size:
                # Array is full - drop the worst level (furthest from best price)
                # This means we drop the last element (index n_levels-1)
                n_levels = max_size - 1

            # Now we have room to insert
            if idx < n_levels:
                prices[idx + 1:n_levels + 1] = prices[idx:n_levels]
                sizes[idx + 1:n_levels + 1] = sizes[idx:n_levels]
            prices[idx] = check_price
            sizes[idx] = size
            return n_levels + 1
        return n_levels

@njit(cache=True, fastmath=True)
def _aggregate_dual_granularity(
    prices: np.ndarray,
    sizes: np.ndarray,
    n_levels: int,
    fine_tick: float,
    coarse_tick: float,
    n_fine_levels: int,
    reverse: bool,
    out_prices: np.ndarray,
    out_sizes: np.ndarray
) -> int:
    """
    Aggregate with dual granularity: fine for first n_fine_levels, coarse beyond.

    Args:
        prices: Raw price levels (negative for bids)
        sizes: Raw sizes
        n_levels: Number of raw levels
        fine_tick: Tick size for first n_fine_levels (e.g., 1.0)
        coarse_tick: Tick size beyond n_fine_levels (e.g., 100.0)
        n_fine_levels: How many raw levels to aggregate with fine_tick
        reverse: True for bids (stored as negative)
        out_prices: Output array for aggregated prices
        out_sizes: Output array for aggregated sizes

    Returns:
        Number of aggregated levels
    """
    if n_levels == 0:
        return 0

    n_agg = 0
    current_bucket = 0.0
    bucket_size = 0.0
    current_tick = fine_tick

    for i in range(n_levels):
        price = -prices[i] if reverse else prices[i]
        size = sizes[i]

        # Switch to coarse tick after n_fine_levels
        if i >= n_fine_levels:
            current_tick = coarse_tick

        # Determine bucket
        bucket = np.floor(price / current_tick) * current_tick

        if i == 0:
            current_bucket = bucket
            bucket_size = size
        elif abs(bucket - current_bucket) < 1e-9:
            # Same bucket, accumulate
            bucket_size += size
        else:
            # New bucket, save previous
            out_prices[n_agg] = current_bucket
            out_sizes[n_agg] = bucket_size
            n_agg += 1

            # Start new bucket
            current_bucket = bucket
            bucket_size = size

    # Save last bucket
    if bucket_size > 0:
        out_prices[n_agg] = current_bucket
        out_sizes[n_agg] = bucket_size
        n_agg += 1

    return n_agg

@njit(cache=True, fastmath=True)
def _aggregate_single_granularity(
    prices: np.ndarray,
    sizes: np.ndarray,
    n_levels: int,
    tick_size: float,
    reverse: bool,
    out_prices: np.ndarray,
    out_sizes: np.ndarray
) -> int:
    """
    Simple single-granularity aggregation.
    Faster than dual when you don't need variable granularity.
    """
    if n_levels == 0:
        return 0

    n_agg = 0
    current_bucket = 0.0
    bucket_size = 0.0

    for i in range(n_levels):
        price = -prices[i] if reverse else prices[i]
        size = sizes[i]

        bucket = np.floor(price / tick_size) * tick_size

        if i == 0:
            current_bucket = bucket
            bucket_size = size
        elif abs(bucket - current_bucket) < 1e-9:
            bucket_size += size
        else:
            out_prices[n_agg] = current_bucket
            out_sizes[n_agg] = bucket_size
            n_agg += 1
            current_bucket = bucket
            bucket_size = size

    if bucket_size > 0:
        out_prices[n_agg] = current_bucket
        out_sizes[n_agg] = bucket_size
        n_agg += 1

    return n_agg

@njit(cache=True, fastmath=True)
def _get_mid_price_fast(
        bid_prices: np.ndarray,
        ask_prices: np.ndarray,
        n_bids: int,
        n_asks: int
) -> float:
    """O(1) mid price calculation"""
    if n_bids == 0 or n_asks == 0:
        return 0.0
    return (-bid_prices[0] + ask_prices[0]) * 0.5

@njit(cache=True, fastmath=True)
def _sweep_price_fast(
        prices: np.ndarray,
        sizes: np.ndarray,
        n_levels: int,
        target_size: float,
        reverse: bool
) -> float:
    """Calculate price to sweep target_size."""
    if n_levels == 0:
        return 0.0

    cumsize = 0.0
    for i in range(n_levels):
        cumsize += sizes[i]
        if cumsize >= target_size:
            return -prices[i] if reverse else prices[i]

    return -prices[n_levels - 1] if reverse else prices[n_levels - 1]

@njit(cache=True, fastmath=True)
def _calculate_depth_fast(
        prices: np.ndarray,
        sizes: np.ndarray,
        n_levels: int,
        depth_price: float,
        reverse: bool
) -> float:
    """Calculate total size from best to depth_price."""
    total = 0.0
    for i in range(n_levels):
        price = -prices[i] if reverse else prices[i]
        if reverse:
            if price >= depth_price:
                total += sizes[i]
            else:
                break
        else:
            if price <= depth_price:
                total += sizes[i]
            else:
                break
    return total




# ============================================================================
# Usage Examples
# ============================================================================

def example_usage():
    """Demonstrate dual-granularity aggregation"""
    import time

    # Create synthetic deep book
    bid_levels = [
        OrderBookLevel(price=100000.0 - i * 0.5, size=np.random.uniform(0.1, 5.0))
        for i in range(1000)
    ]
    ask_levels = [
        OrderBookLevel(price=100000.0 + i * 0.5, size=np.random.uniform(0.1, 5.0))
        for i in range(1000)
    ]

    snapshot = L2Snapshot(
        timestamp=int(time.time() * 1000),
        symbol="BTC-PERP",
        bids=bid_levels,
        asks=ask_levels
    )

    book = FastOrderBook("BTC-PERP", max_levels=2000)
    book.from_snapshot(snapshot)

    print(f"Book: {book}")
    print(f"Mid: {book.get_mid_price():.2f}")
    print(f"Spread: {book.get_spread():.4f}")

    # Dual granularity: first 50 levels at $1, rest at $100
    start = time.perf_counter()
    for _ in range(1000):
        bid_prices, bid_sizes = book.aggregate_dual(Side.BUY, 1.0, 100.0, 50)
    dual_time = (time.perf_counter() - start) * 1e6 / 1000

    print(f"\nDual aggregation (50@$1, rest@$100): {dual_time:.1f} μs")
    print(f"  Aggregated to {len(bid_prices)} levels")
    print(f"  First 5 levels: {list(zip(bid_prices[:5], bid_sizes[:5]))}")
    print(f"  Last 5 levels: {list(zip(bid_prices[-5:], bid_sizes[-5:]))}")

    # Single granularity for comparison
    start = time.perf_counter()
    for _ in range(1000):
        bid_prices, bid_sizes = book.aggregate(Side.BUY, 10.0)
    single_time = (time.perf_counter() - start) * 1e6 / 1000

    print(f"\nSingle aggregation (@$10): {single_time:.1f} μs")
    print(f"  Aggregated to {len(bid_prices)} levels")


if __name__ == "__main__":
    example_usage()