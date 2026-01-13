import time

import numpy as np
from numba import njit, types
from typing import Optional, Tuple
from bulk.common import Side
from bulk.messages import L2Snapshot, OrderBookLevel, L2Delta


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
# Tests
# ============================================================================

def test_snapshot_loading():
    """
    Test 1: Load full snapshot and verify book state
    """
    print("\n" + "=" * 80)
    print("TEST 1: Snapshot Loading")
    print("=" * 80)

    # Create a snapshot with 20 bid levels and 20 ask levels
    bid_levels = [
        OrderBookLevel(price=100000.0 - i * 10.0, size=float(i + 1) * 0.5, num_orders=i + 1)
        for i in range(600)
    ]
    ask_levels = [
        OrderBookLevel(price=100010.0 + i * 10.0, size=float(i + 1) * 0.5, num_orders=i + 1)
        for i in range(600)
    ]

    snapshot = L2Snapshot(
        timestamp=int(time.time() * 1000),
        symbol="BTC-USD",
        bids=bid_levels,
        asks=ask_levels
    )

    # Create book and load snapshot
    book = FastOrderBook("BTC-USD", max_levels=1000)
    book.from_snapshot(snapshot)

    print(f"\nBook initialized: {book}")

    # Test 1.1: Verify number of levels
    assert book.n_bids == 600, f"Expected 20 bids, got {book.n_bids}"
    assert book.n_asks == 600, f"Expected 20 asks, got {book.n_asks}"
    print(f"✓ Correct number of levels: {book.n_bids} bids, {book.n_asks} asks")

    # Test 1.2: Verify best bid/ask
    best_bid = book.get_best_bid()
    best_ask = book.get_best_ask()
    assert best_bid == 100000.0, f"Expected best bid 100000.0, got {best_bid}"
    assert best_ask == 100010.0, f"Expected best ask 100010.0, got {best_ask}"
    print(f"✓ Best bid: ${best_bid:,.2f}, Best ask: ${best_ask:,.2f}")

    # Test 1.3: Verify mid price and spread
    mid = book.get_mid_price()
    spread = book.get_spread()
    expected_mid = (100000.0 + 100010.0) / 2.0
    expected_spread = 10.0
    assert abs(mid - expected_mid) < 1e-6, f"Expected mid {expected_mid}, got {mid}"
    assert abs(spread - expected_spread) < 1e-6, f"Expected spread {expected_spread}, got {spread}"
    print(f"✓ Mid: ${mid:,.2f}, Spread: ${spread:.2f}")

    # Test 1.4: Verify bid array sorting (highest to lowest)
    bid_prices, bid_sizes = book.get_bids_array()
    print(f"\n✓ Bid array retrieved: {len(bid_prices)} levels")
    print(f"  First 5 bids:")
    for i in range(min(5, len(bid_prices))):
        print(f"    {i + 1}. ${bid_prices[i]:,.2f} x {bid_sizes[i]:.4f}")

    # Verify bids are sorted descending (highest to lowest)
    for i in range(len(bid_prices) - 1):
        assert bid_prices[i] > bid_prices[i + 1], \
            f"Bids not sorted: {bid_prices[i]} should be > {bid_prices[i + 1]}"
    print(f"  ✓ Bids properly sorted (highest to lowest)")

    # Verify bid prices and sizes match expected
    for i in range(len(bid_prices)):
        expected_price = 100000.0 - i * 10.0
        expected_size = (i + 1) * 0.5
        assert abs(bid_prices[i] - expected_price) < 1e-6, \
            f"Bid price mismatch at {i}: expected {expected_price}, got {bid_prices[i]}"
        assert abs(bid_sizes[i] - expected_size) < 1e-6, \
            f"Bid size mismatch at {i}: expected {expected_size}, got {bid_sizes[i]}"
    print(f"  ✓ All bid prices and sizes match snapshot")

    # Test 1.5: Verify ask array sorting (lowest to highest)
    ask_prices, ask_sizes = book.get_asks_array()
    print(f"\n✓ Ask array retrieved: {len(ask_prices)} levels")
    print(f"  First 5 asks:")
    for i in range(min(5, len(ask_prices))):
        print(f"    {i + 1}. ${ask_prices[i]:,.2f} x {ask_sizes[i]:.4f}")

    # Verify asks are sorted ascending (lowest to highest)
    for i in range(len(ask_prices) - 1):
        assert ask_prices[i] < ask_prices[i + 1], \
            f"Asks not sorted: {ask_prices[i]} should be < {ask_prices[i + 1]}"
    print(f"  ✓ Asks properly sorted (lowest to highest)")

    # Verify ask prices and sizes match expected
    for i in range(len(ask_prices)):
        expected_price = 100010.0 + i * 10.0
        expected_size = (i + 1) * 0.5
        assert abs(ask_prices[i] - expected_price) < 1e-6, \
            f"Ask price mismatch at {i}: expected {expected_price}, got {ask_prices[i]}"
        assert abs(ask_sizes[i] - expected_size) < 1e-6, \
            f"Ask size mismatch at {i}: expected {expected_size}, got {ask_sizes[i]}"
    print(f"  ✓ All ask prices and sizes match snapshot")

    print(f"\n✓ TEST 1 PASSED: Snapshot loading works correctly")
    return book


def test_delta_updates(book: FastOrderBook):
    """
    Test 2: Apply deltas and verify book updates correctly
    """
    print("\n" + "=" * 80)
    print("TEST 2: Delta Updates")
    print("=" * 80)

    # Get initial state
    initial_bids, _ = book.get_bids_array()
    initial_asks, _ = book.get_asks_array()
    print(f"\nInitial state: {book.n_bids} bids, {book.n_asks} asks")
    print(f"  Best bid: ${book.get_best_bid():,.2f}")
    print(f"  Best ask: ${book.get_best_ask():,.2f}")

    # Test 2.1: Update existing bid level
    print("\n--- Test 2.1: Update existing bid level ---")
    update_price = 99990.0  # This is the second bid level
    new_size = 5.0

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[OrderBookLevel(price=update_price, size=new_size)],
        ask_changes=[],
        timestamp=int(time.time() * 1000)
    )
    book.apply_delta(delta)

    bid_prices, bid_sizes = book.get_bids_array()
    # Find the updated level
    idx = np.where(np.abs(bid_prices - update_price) < 1e-6)[0]
    assert len(idx) > 0, f"Updated bid level {update_price} not found"
    assert abs(bid_sizes[idx[0]] - new_size) < 1e-6, \
        f"Bid size not updated: expected {new_size}, got {bid_sizes[idx[0]]}"
    print(f"✓ Updated bid at ${update_price:,.2f} to size {new_size:.4f}")

    # Test 2.2: Add new bid level (better than current best)
    print("\n--- Test 2.2: Add new best bid ---")
    new_best_bid = 100001.0
    new_bid_size = 2.5

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[OrderBookLevel(price=new_best_bid, size=new_bid_size)],
        ask_changes=[],
        timestamp=int(time.time() * 1000)
    )
    book.apply_delta(delta)

    assert book.get_best_bid() == new_best_bid, \
        f"New best bid not correct: expected {new_best_bid}, got {book.get_best_bid()}"

    bid_prices, bid_sizes = book.get_bids_array()
    assert bid_prices[0] == new_best_bid, f"New bid not at top: {bid_prices[0]}"
    assert abs(bid_sizes[0] - new_bid_size) < 1e-6, \
        f"New bid size wrong: expected {new_bid_size}, got {bid_sizes[0]}"
    print(f"✓ New best bid: ${new_best_bid:,.2f} x {new_bid_size:.4f}")
    print(f"  Book now has {book.n_bids} bid levels")

    # Test 2.3: Delete a bid level (size=0)
    print("\n--- Test 2.3: Delete bid level ---")
    delete_price = 99980.0  # Delete one of the middle levels

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[OrderBookLevel(price=delete_price, size=0.0)],
        ask_changes=[],
        timestamp=int(time.time() * 1000)
    )
    n_bids_before = book.n_bids
    book.apply_delta(delta)
    n_bids_after = book.n_bids

    assert n_bids_after == n_bids_before - 1, \
        f"Level not deleted: expected {n_bids_before - 1} bids, got {n_bids_after}"

    bid_prices, _ = book.get_bids_array()
    assert not np.any(np.abs(bid_prices - delete_price) < 1e-6), \
        f"Deleted level {delete_price} still in book"
    print(f"✓ Deleted bid at ${delete_price:,.2f}")
    print(f"  Book now has {book.n_bids} bid levels")

    # Test 2.4: Update existing ask level
    print("\n--- Test 2.4: Update existing ask level ---")
    update_ask_price = 100020.0  # Second ask level
    new_ask_size = 7.5

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[],
        ask_changes=[OrderBookLevel(price=update_ask_price, size=new_ask_size)],
        timestamp=int(time.time() * 1000)
    )
    book.apply_delta(delta)

    ask_prices, ask_sizes = book.get_asks_array()
    idx = np.where(np.abs(ask_prices - update_ask_price) < 1e-6)[0]
    assert len(idx) > 0, f"Updated ask level {update_ask_price} not found"
    assert abs(ask_sizes[idx[0]] - new_ask_size) < 1e-6, \
        f"Ask size not updated: expected {new_ask_size}, got {ask_sizes[idx[0]]}"
    print(f"✓ Updated ask at ${update_ask_price:,.2f} to size {new_ask_size:.4f}")

    # Test 2.5: Add new ask level (better than current best)
    print("\n--- Test 2.5: Add new best ask ---")
    new_best_ask = 100009.0
    new_ask_size = 3.5

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[],
        ask_changes=[OrderBookLevel(price=new_best_ask, size=new_ask_size)],
        timestamp=int(time.time() * 1000)
    )
    book.apply_delta(delta)

    assert book.get_best_ask() == new_best_ask, \
        f"New best ask not correct: expected {new_best_ask}, got {book.get_best_ask()}"

    ask_prices, ask_sizes = book.get_asks_array()
    assert ask_prices[0] == new_best_ask, f"New ask not at top: {ask_prices[0]}"
    assert abs(ask_sizes[0] - new_ask_size) < 1e-6, \
        f"New ask size wrong: expected {new_ask_size}, got {ask_sizes[0]}"
    print(f"✓ New best ask: ${new_best_ask:,.2f} x {new_ask_size:.4f}")
    print(f"  Book now has {book.n_asks} ask levels")

    # Test 2.6: Delete an ask level
    print("\n--- Test 2.6: Delete ask level ---")
    delete_ask_price = 100030.0

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=[],
        ask_changes=[OrderBookLevel(price=delete_ask_price, size=0.0)],
        timestamp=int(time.time() * 1000)
    )
    n_asks_before = book.n_asks
    book.apply_delta(delta)
    n_asks_after = book.n_asks

    assert n_asks_after == n_asks_before - 1, \
        f"Level not deleted: expected {n_asks_before - 1} asks, got {n_asks_after}"

    ask_prices, _ = book.get_asks_array()
    assert not np.any(np.abs(ask_prices - delete_ask_price) < 1e-6), \
        f"Deleted level {delete_ask_price} still in book"
    print(f"✓ Deleted ask at ${delete_ask_price:,.2f}")
    print(f"  Book now has {book.n_asks} ask levels")

    # Test 2.7: Multi-level delta (update multiple levels at once)
    print("\n--- Test 2.7: Multi-level delta update ---")
    bid_changes = [
        OrderBookLevel(price=99970.0, size=10.0),
        OrderBookLevel(price=99960.0, size=12.0),
        OrderBookLevel(price=99950.0, size=0.0),  # Delete this one
    ]
    ask_changes = [
        OrderBookLevel(price=100040.0, size=8.0),
        OrderBookLevel(price=100050.0, size=9.0),
    ]

    delta = L2Delta(
        symbol="BTC-USD",
        bid_changes=bid_changes,
        ask_changes=ask_changes,
        timestamp=int(time.time() * 1000)
    )
    book.apply_delta(delta)

    bid_prices, bid_sizes = book.get_bids_array()
    ask_prices, ask_sizes = book.get_asks_array()

    # Verify bid changes
    idx = np.where(np.abs(bid_prices - 99970.0) < 1e-6)[0]
    assert len(idx) > 0 and abs(bid_sizes[idx[0]] - 10.0) < 1e-6, "Bid 99970 not updated"

    idx = np.where(np.abs(bid_prices - 99960.0) < 1e-6)[0]
    assert len(idx) > 0 and abs(bid_sizes[idx[0]] - 12.0) < 1e-6, "Bid 99960 not updated"

    assert not np.any(np.abs(bid_prices - 99950.0) < 1e-6), "Bid 99950 not deleted"

    # Verify ask changes
    idx = np.where(np.abs(ask_prices - 100040.0) < 1e-6)[0]
    assert len(idx) > 0 and abs(ask_sizes[idx[0]] - 8.0) < 1e-6, "Ask 100040 not updated"

    idx = np.where(np.abs(ask_prices - 100050.0) < 1e-6)[0]
    assert len(idx) > 0 and abs(ask_sizes[idx[0]] - 9.0) < 1e-6, "Ask 100050 not updated"

    print(f"✓ Multi-level delta applied successfully")
    print(f"  Updated 3 bid levels (2 updates, 1 delete)")
    print(f"  Updated 2 ask levels")

    # Test 2.8: Verify sorting maintained after all updates
    print("\n--- Test 2.8: Verify sorting integrity ---")
    bid_prices, _ = book.get_bids_array()
    ask_prices, _ = book.get_asks_array()

    # Check bids are still sorted descending
    for i in range(len(bid_prices) - 1):
        assert bid_prices[i] > bid_prices[i + 1], \
            f"Bid sorting broken at {i}: {bid_prices[i]} should be > {bid_prices[i + 1]}"
    print(f"✓ Bids still properly sorted (highest to lowest)")

    # Check asks are still sorted ascending
    for i in range(len(ask_prices) - 1):
        assert ask_prices[i] < ask_prices[i + 1], \
            f"Ask sorting broken at {i}: {ask_prices[i]} should be < {ask_prices[i + 1]}"
    print(f"✓ Asks still properly sorted (lowest to highest)")

    # Verify spread is positive
    spread = book.get_spread()
    assert spread > 0, f"Spread should be positive, got {spread}"
    print(f"✓ Spread is positive: ${spread:.2f}")

    # Final state
    print(f"\n--- Final book state ---")
    print(f"  {book}")
    print(f"  Best bid: ${book.get_best_bid():,.2f}")
    print(f"  Best ask: ${book.get_best_ask():,.2f}")
    print(f"  Mid: ${book.get_mid_price():,.2f}")
    print(f"  Spread: ${book.get_spread():.2f}")

    # Show top 5 levels
    bid_prices, bid_sizes = book.get_bids_array(n=5)
    ask_prices, ask_sizes = book.get_asks_array(n=5)

    print(f"\n  Top 5 bids:")
    for i in range(len(bid_prices)):
        print(f"    {i + 1}. ${bid_prices[i]:,.2f} x {bid_sizes[i]:.4f}")

    print(f"\n  Top 5 asks:")
    for i in range(len(ask_prices)):
        print(f"    {i + 1}. ${ask_prices[i]:,.2f} x {ask_sizes[i]:.4f}")

    print(f"\n✓ TEST 2 PASSED: Delta updates work correctly")


def test_array_retrieval_with_limit():
    """
    Test 3: Verify get_bids_array and get_asks_array with n parameter
    """
    print("\n" + "=" * 80)
    print("TEST 3: Array Retrieval with Limit")
    print("=" * 80)

    # Create book with 50 levels per side
    bid_levels = [
        OrderBookLevel(price=50000.0 - i * 5.0, size=float(i + 1) * 0.1)
        for i in range(50)
    ]
    ask_levels = [
        OrderBookLevel(price=50100.0 + i * 5.0, size=float(i + 1) * 0.1)
        for i in range(50)
    ]

    snapshot = L2Snapshot(
        timestamp=int(time.time() * 1000),
        symbol="ETH-USD",
        bids=bid_levels,
        asks=ask_levels
    )

    book = FastOrderBook("ETH-USD", max_levels=1000)
    book.from_snapshot(snapshot)

    print(f"\nBook initialized with 50 levels per side")

    # Test 3.1: Get all levels (no limit)
    print("\n--- Test 3.1: Get all levels ---")
    all_bid_prices, all_bid_sizes = book.get_bids_array()
    all_ask_prices, all_ask_sizes = book.get_asks_array()

    assert len(all_bid_prices) == 50, f"Expected 50 bids, got {len(all_bid_prices)}"
    assert len(all_ask_prices) == 50, f"Expected 50 asks, got {len(all_ask_prices)}"
    print(f"✓ Retrieved all levels: {len(all_bid_prices)} bids, {len(all_ask_prices)} asks")

    # Test 3.2: Get limited levels
    print("\n--- Test 3.2: Get top 10 levels ---")
    top10_bid_prices, top10_bid_sizes = book.get_bids_array(n=10)
    top10_ask_prices, top10_ask_sizes = book.get_asks_array(n=10)

    assert len(top10_bid_prices) == 10, f"Expected 10 bids, got {len(top10_bid_prices)}"
    assert len(top10_ask_prices) == 10, f"Expected 10 asks, got {len(top10_ask_prices)}"
    print(f"✓ Retrieved top 10 levels per side")

    # Verify these are the best 10 levels
    assert np.allclose(top10_bid_prices, all_bid_prices[:10]), "Top 10 bids don't match"
    assert np.allclose(top10_ask_prices, all_ask_prices[:10]), "Top 10 asks don't match"
    print(f"✓ Top 10 levels match first 10 from full array")

    # Test 3.3: Get 1 level (best only)
    print("\n--- Test 3.3: Get best level only ---")
    best_bid_prices, best_bid_sizes = book.get_bids_array(n=1)
    best_ask_prices, best_ask_sizes = book.get_asks_array(n=1)

    assert len(best_bid_prices) == 1, f"Expected 1 bid, got {len(best_bid_prices)}"
    assert len(best_ask_prices) == 1, f"Expected 1 ask, got {len(best_ask_prices)}"
    assert best_bid_prices[0] == book.get_best_bid(), "Best bid doesn't match"
    assert best_ask_prices[0] == book.get_best_ask(), "Best ask doesn't match"
    print(f"✓ Single level retrieval works correctly")
    print(f"  Best bid: ${best_bid_prices[0]:,.2f}")
    print(f"  Best ask: ${best_ask_prices[0]:,.2f}")

    # Test 3.4: Request more levels than available
    print("\n--- Test 3.4: Request more levels than available ---")
    oversize_bids, _ = book.get_bids_array(n=100)
    oversize_asks, _ = book.get_asks_array(n=100)

    assert len(oversize_bids) == 50, \
        f"Should return 50 bids when requesting 100, got {len(oversize_bids)}"
    assert len(oversize_asks) == 50, \
        f"Should return 50 asks when requesting 100, got {len(oversize_asks)}"
    print(f"✓ Correctly handles oversized requests (returns available levels)")

    print(f"\n✓ TEST 3 PASSED: Array retrieval with limits works correctly")


def run_all_tests():
    """Run all test cases"""
    print("\n" + "=" * 80)
    print("FASTORDERBOOK TEST SUITE")
    print("=" * 80)
    print(f"Testing snapshot loading, delta updates, and array retrieval")

    try:
        # Test 1: Snapshot loading
        book = test_snapshot_loading()

        # Test 2: Delta updates (uses book from Test 1)
        test_delta_updates(book)

        # Test 3: Array retrieval with limits
        test_array_retrieval_with_limit()

        # Summary
        print("\n" + "=" * 80)
        print("ALL TESTS PASSED ✓")
        print("=" * 80)
        print("\nSummary:")
        print("  ✓ Snapshot loading works correctly")
        print("  ✓ Bid/ask arrays properly sorted")
        print("  ✓ Delta updates (add/update/delete) work correctly")
        print("  ✓ Multi-level deltas work correctly")
        print("  ✓ Sorting maintained after updates")
        print("  ✓ Array retrieval with limits works correctly")
        print("\n" + "=" * 80)

    except AssertionError as e:
        print(f"\n❌ TEST FAILED: {e}")
        raise
    except Exception as e:
        print(f"\n❌ UNEXPECTED ERROR: {e}")
        raise


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
    run_all_tests()
    example_usage()