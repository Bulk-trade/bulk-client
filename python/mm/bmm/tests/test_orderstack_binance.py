"""
OrderStack Simulator with Binance Feed

Tests OrderStack functionality by:
- Subscribing to Binance BTCUSDT with 1000 levels
- Maintaining bid and ask OrderStacks
- Periodically sampling and syncing with Binance book (1 second intervals)
- Using simulated execution (no actual orders)
"""

import asyncio
import logging
import time
from typing import Optional, Dict
from dataclasses import dataclass
import numpy as np

# Add project root to path
import sys
from pathlib import Path

from common import TransactionSigner

project_root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(project_root))

from mm.bmm.adaptors import BinanceFuturesWebSocketClient, Topic
from mm.bmm.common import OrderStack
from bulk.common import Side
from bulk.messages.md import L2Delta


@dataclass
class SyncStats:
    """Statistics for a single sync operation"""
    timestamp: float
    side: str
    placed: int
    cancelled: int
    total_size: float
    num_orders: int
    num_levels: int


class OrderStackSimulator:
    """
    Test harness for OrderStack with live Binance data feed

    Features:
    - Connects to Binance BTCUSDT futures (1000 levels)
    - Maintains bid and ask OrderStacks
    - Periodic sync every 1 second
    - Simulated execution (no real orders)
    - Statistics and monitoring
    """

    def __init__(
            self,
            symbol: str = "BTCUSDT",
            chunk_size: float = 0.25,
            max_orders: int = 5,
            max_price_levels: int = 50,
            sync_interval: float = 1.0,
            logger: Optional[logging.Logger] = None
    ):
        """
        Initialize simulator

        Args:
            symbol: Binance symbol (e.g., "BTCUSDT")
            chunk_size: Order size for OrderStack
            max_orders: Maximum orders per level
            max_price_levels: Number of price levels to track
            sync_interval: Sync interval in seconds
            logger: Logger instance
        """
        self.symbol = symbol
        self.chunk_size = chunk_size
        self.max_orders = max_orders
        self.max_price_levels = max_price_levels
        self.sync_interval = sync_interval
        self.logger = logger or logging.getLogger(__name__)

        # Binance client
        self.binance_client: Optional[BinanceFuturesWebSocketClient] = None

        # OrderStacks
        self.bid_stack: Optional[OrderStack] = None
        self.ask_stack: Optional[OrderStack] = None

        # Monitoring
        self.delta_count = 0
        self.sync_count = 0
        self.sync_history: list[SyncStats] = []
        self.last_sync_time = 0.0
        self.start_time = 0.0

        # Tasks
        self.sync_task: Optional[asyncio.Task] = None

    async def setup(self) -> bool:
        """
        Initialize Binance client and OrderStacks

        Returns:
            True if setup successful
        """
        try:
            self.logger.info("=" * 80)
            self.logger.info("ORDERSTACK SIMULATOR SETUP")
            self.logger.info("=" * 80)

            # Create Binance client
            self.logger.info(f"\n1. Connecting to Binance {self.symbol}...")
            self.binance_client = BinanceFuturesWebSocketClient(
                symbols=[self.symbol],
                snapshot_limit=1000,
                enable_bbo=False,
                logger=self.logger,
                handlers={
                    Topic.L2_DELTA: self._handle_delta,
                }
            )

            signer = TransactionSigner.generate_account()

            # Connect to Binance
            connected = await self.binance_client.connect()
            if not connected:
                self.logger.error("Failed to connect to Binance")
                return False

            self.logger.info("   ✓ Connected to Binance")

            # Wait for initial book
            self.logger.info("   Waiting for order book initialization...")
            await asyncio.sleep(2)

            # Verify we have book data
            book = self.binance_client.get_book(self.symbol)
            if not book:
                self.logger.error("Order book not initialized")
                return False

            mid = book.get_mid_price()
            spread = book.get_spread()
            self.logger.info(f"   ✓ Book initialized: Mid=${mid:.2f}, Spread=${spread:.4f}")

            # Create OrderStacks
            self.logger.info(f"\n2. Creating OrderStacks...")
            self.logger.info(f"   Chunk size: {self.chunk_size}")
            self.logger.info(f"   Max orders per level: {self.max_orders}")
            self.logger.info(f"   Max price levels: {self.max_price_levels}")

            self.bid_stack = OrderStack(
                symbol=self.symbol,
                side=Side.BUY,
                chunk_size=self.chunk_size,
                max_orders=self.max_orders,
                max_price_levels=self.max_price_levels,
                signer=signer,
            )

            self.ask_stack = OrderStack(
                symbol=self.symbol,
                side=Side.SELL,
                chunk_size=self.chunk_size,
                max_orders=self.max_orders,
                max_price_levels=self.max_price_levels,
                signer=signer,
            )

            self.logger.info("   ✓ OrderStacks created")

            # Perform initial sync
            self.logger.info(f"\n3. Performing initial sync...")
            await self._sync_with_binance()
            self.logger.info("   ✓ Initial sync complete")

            # Start periodic sync
            self.logger.info(f"\n4. Starting periodic sync (interval={self.sync_interval}s)...")
            self.sync_task = asyncio.create_task(self._periodic_sync())
            self.start_time = time.time()
            self.logger.info("   ✓ Periodic sync started")

            self.logger.info("\n" + "=" * 80)
            self.logger.info("SIMULATOR READY")
            self.logger.info("=" * 80 + "\n")

            return True

        except Exception as e:
            self.logger.error(f"Setup failed: {e}", exc_info=True)
            return False

    async def _periodic_sync(self):
        """Periodic sync loop"""
        while True:
            try:
                await asyncio.sleep(self.sync_interval)
                await self._sync_with_binance()

            except asyncio.CancelledError:
                self.logger.info("Periodic sync cancelled")
                break

            except Exception as e:
                self.logger.error(f"Sync error: {e}", exc_info=True)

    async def _sync_with_binance(self):
        """Sample Binance book and sync OrderStacks"""
        try:
            book = self.binance_client.get_book(self.symbol)
            if not book:
                self.logger.warning("No book data available")
                return

            # Get bid and ask data (limit to max_price_levels)
            bid_prices, bid_sizes = book.get_bids_array(n=self.max_price_levels)
            ask_prices, ask_sizes = book.get_asks_array(n=self.max_price_levels)

            if len(bid_prices) == 0 or len(ask_prices) == 0:
                self.logger.warning("Empty book data")
                return

            # Sync bid stack (simulated - ws_client=None)
            placed_bids, cancelled_bids, success_bid = await self.bid_stack.execute(
                ws_client=None,
                new_prices=bid_prices,
                new_sizes=bid_sizes
            )

            # Sync ask stack (simulated - ws_client=None)
            placed_asks, cancelled_asks, success_ask = await self.ask_stack.execute(
                ws_client=None,
                new_prices=ask_prices,
                new_sizes=ask_sizes
            )

            # Record statistics
            self.sync_count += 1
            self.last_sync_time = time.time()

            # Record bid stats
            bid_stats = self.bid_stack.get_stats()
            self.sync_history.append(SyncStats(
                timestamp=self.last_sync_time,
                side="BID",
                placed=len(placed_bids),
                cancelled=len(cancelled_bids),
                total_size=bid_stats['total_size'],
                num_orders=bid_stats['num_active_orders'],
                num_levels=bid_stats['num_levels']
            ))

            # Record ask stats
            ask_stats = self.ask_stack.get_stats()
            self.sync_history.append(SyncStats(
                timestamp=self.last_sync_time,
                side="ASK",
                placed=len(placed_asks),
                cancelled=len(cancelled_asks),
                total_size=ask_stats['total_size'],
                num_orders=ask_stats['num_active_orders'],
                num_levels=ask_stats['num_levels']
            ))

            # Log periodic statistics
            if self.sync_count % 10 == 0:
                self._log_stats()

        except Exception as e:
            self.logger.error(f"Sync execution error: {e}", exc_info=True)

    def _handle_delta(self, client: BinanceFuturesWebSocketClient, delta: L2Delta):
        """Handle delta updates from Binance (for monitoring)"""
        self.delta_count += 1

    def _log_stats(self):
        """Log current statistics"""
        elapsed = time.time() - self.start_time

        bid_stats = self.bid_stack.get_stats()
        ask_stats = self.ask_stack.get_stats()

        # Get recent sync activity
        recent = self.sync_history[-20:] if len(self.sync_history) > 20 else self.sync_history
        total_placed = sum(s.placed for s in recent)
        total_cancelled = sum(s.cancelled for s in recent)

        # Get book state
        book = self.binance_client.get_book(self.symbol)
        mid = book.get_mid_price() if book else 0.0
        spread = book.get_spread() if book else 0.0

        self.logger.info("\n" + "=" * 80)
        self.logger.info(f"STATISTICS - Sync #{self.sync_count} ({elapsed:.1f}s elapsed)")
        self.logger.info("=" * 80)

        self.logger.info(f"\nBinance Book:")
        self.logger.info(f"  Mid Price:    ${mid:,.2f}")
        self.logger.info(f"  Spread:       ${spread:.4f}")
        self.logger.info(f"  Delta Updates: {self.delta_count}")

        self.logger.info(f"\nBid Stack:")
        self.logger.info(f"  Levels:       {bid_stats['num_levels']}")
        self.logger.info(f"  Active Orders: {bid_stats['num_active_orders']}")
        self.logger.info(f"  Total Size:   {bid_stats['total_size']:.4f}")

        self.logger.info(f"\nAsk Stack:")
        self.logger.info(f"  Levels:       {ask_stats['num_levels']}")
        self.logger.info(f"  Active Orders: {ask_stats['num_active_orders']}")
        self.logger.info(f"  Total Size:   {ask_stats['total_size']:.4f}")

        self.logger.info(f"\nRecent Activity (last {len(recent)} syncs):")
        self.logger.info(f"  Orders Placed:    {total_placed}")
        self.logger.info(f"  Orders Cancelled: {total_cancelled}")

        # Show discrepancy
        if book:
            bid_prices, bid_sizes = book.get_bids_array(n=5)
            ask_prices, ask_sizes = book.get_asks_array(n=5)

            bid_disc = self.bid_stack.get_total_discrepancy(bid_prices, bid_sizes)
            ask_disc = self.ask_stack.get_total_discrepancy(ask_prices, ask_sizes)

            self.logger.info(f"\nDiscrepancy (top 5 levels):")
            self.logger.info(f"  Bid Total: {bid_disc:.6f}")
            self.logger.info(f"  Ask Total: {ask_disc:.6f}")

        self.logger.info("=" * 80 + "\n")

    def print_detailed_book_comparison(self, n_levels: int = 10):
        """Print detailed comparison of OrderStack vs Binance book"""
        book = self.binance_client.get_book(self.symbol)
        if not book:
            self.logger.warning("No book data available")
            return

        # Get Binance data
        bid_prices, bid_sizes = book.get_bids_array(n=n_levels)
        ask_prices, ask_sizes = book.get_asks_array(n=n_levels)

        # Get OrderStack data
        bid_levels = self.bid_stack.get_level_summary()
        ask_levels = self.ask_stack.get_level_summary()

        # Get discrepancies
        bid_disc = self.bid_stack.get_discrepancy(bid_prices, bid_sizes)
        ask_disc = self.ask_stack.get_discrepancy(ask_prices, ask_sizes)

        print("\n" + "=" * 100)
        print(f"DETAILED BOOK COMPARISON - {self.symbol}")
        print("=" * 100)

        print("\nBID SIDE:")
        print(f"{'Price':<12} {'Binance':<12} {'Stack':<12} {'Orders':<8} {'Discrepancy':<12}")
        print("-" * 100)
        for i in range(min(n_levels, len(bid_prices))):
            price = bid_prices[i]
            binance_size = bid_sizes[i]
            stack_size, n_orders = bid_levels.get(price, (0.0, 0))
            disc = bid_disc.get(price, 0.0)

            print(f"${price:<11,.2f} {binance_size:<12.4f} {stack_size:<12.4f} "
                  f"{n_orders:<8} {disc:+.6f}")

        print("\nASK SIDE:")
        print(f"{'Price':<12} {'Binance':<12} {'Stack':<12} {'Orders':<8} {'Discrepancy':<12}")
        print("-" * 100)
        for i in range(min(n_levels, len(ask_prices))):
            price = ask_prices[i]
            binance_size = ask_sizes[i]
            stack_size, n_orders = ask_levels.get(price, (0.0, 0))
            disc = ask_disc.get(price, 0.0)

            print(f"${price:<11,.2f} {binance_size:<12.4f} {stack_size:<12.4f} "
                  f"{n_orders:<8} {disc:+.6f}")

        print("=" * 100 + "\n")

    async def run(self, duration_seconds: int = 60):
        """
        Run the simulator for a specified duration

        Args:
            duration_seconds: How long to run in seconds
        """
        self.logger.info(f"Running simulator for {duration_seconds} seconds...")

        try:
            await asyncio.sleep(duration_seconds)
        except KeyboardInterrupt:
            self.logger.info("\nInterrupted by user")

    async def cleanup(self):
        """Cleanup resources"""
        self.logger.info("\nCleaning up...")

        # Stop periodic sync
        if self.sync_task:
            self.sync_task.cancel()
            try:
                await self.sync_task
            except asyncio.CancelledError:
                pass

        # Disconnect from Binance
        if self.binance_client:
            await self.binance_client.disconnect()

        self.logger.info("✓ Cleanup complete")

    def get_summary(self) -> Dict:
        """
        Get summary statistics

        Returns:
            Dictionary with summary statistics
        """
        elapsed = time.time() - self.start_time if self.start_time > 0 else 0

        return {
            'elapsed_time': elapsed,
            'sync_count': self.sync_count,
            'delta_count': self.delta_count,
            'syncs_per_second': self.sync_count / elapsed if elapsed > 0 else 0,
            'deltas_per_second': self.delta_count / elapsed if elapsed > 0 else 0,
            'bid_stats': self.bid_stack.get_stats() if self.bid_stack else {},
            'ask_stats': self.ask_stack.get_stats() if self.ask_stack else {},
        }


async def main():
    """Main test function"""
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Create simulator
    simulator = OrderStackSimulator(
        symbol="BTCUSDT",
        chunk_size=0.25,
        max_orders=5,
        max_price_levels=50,
        sync_interval=1.0,
        logger=logger
    )

    try:
        # Setup
        success = await simulator.setup()
        if not success:
            logger.error("Setup failed")
            return

        # Run for 60 seconds
        await simulator.run(duration_seconds=60)

        # Print final statistics
        logger.info("\n" + "=" * 80)
        logger.info("FINAL STATISTICS")
        logger.info("=" * 80)

        summary = simulator.get_summary()
        for key, value in summary.items():
            if isinstance(value, dict):
                logger.info(f"\n{key}:")
                for k, v in value.items():
                    logger.info(f"  {k}: {v}")
            else:
                logger.info(f"{key}: {value}")

        # Print detailed comparison
        simulator.print_detailed_book_comparison(n_levels=10)

        logger.info("\n✓ Test completed successfully")

    except KeyboardInterrupt:
        logger.info("\n\nTest interrupted by user")

    except Exception as e:
        logger.error(f"\n\nTest failed with exception: {e}", exc_info=True)

    finally:
        # Cleanup
        await simulator.cleanup()


if __name__ == "__main__":
    asyncio.run(main())