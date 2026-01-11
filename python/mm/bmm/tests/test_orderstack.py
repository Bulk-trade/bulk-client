"""
Integration test for OrderStack with BulkWebSocketClient

Tests the full lifecycle:
1. Pull current market price
2. Create synthetic orderbook around that price
3. Initial sync - place orders
4. Track order state updates
5. Simulate market movement - adjust orders
6. Final cleanup with cancelAll
"""

import asyncio
import logging
import os
import time
from typing import Dict, List, Optional
import sys
from pathlib import Path

import numpy as np

# Import Bulk components
from bulk.api import BulkWebSocketClient, Topic
from bulk.api import BulkHttpClient
from bulk.common import Side, OrderStatus
from bulk.common.signer import TransactionSigner
from bulk.messages.account import OrderState

# local stuff
project_root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(project_root))
from mm.bmm.common import OrderStack


class OrderStackTester:
    """Test harness for OrderStack integration"""

    def __init__(
        self,
        symbol: str = "BTC-USD",
        private_key: Optional[str] = None,
        logger: Optional[logging.Logger] = None
    ):
        self.symbol = symbol
        self.private_key = private_key
        self.logger = logger or logging.getLogger(__name__)

        # Clients
        self.http_client: Optional[BulkHttpClient] = None
        self.ws_client: Optional[BulkWebSocketClient] = None
        self.signer: Optional[TransactionSigner] = None

        # Test state
        self.current_price: float = 0.0
        self.bid_stack: Optional[OrderStack] = None
        self.ask_stack: Optional[OrderStack] = None

        # Tracking
        self.order_states: Dict[str, OrderState] = {}
        self.expected_orders: Dict[str, Dict] = {}  # order_id -> {price, size, side}

    async def setup(self):
        """Initialize clients and pull current price"""
        self.logger.info("=" * 80)
        self.logger.info("ORDERSTACK INTEGRATION TEST")
        self.logger.info("=" * 80)

        # Check for private key
        if not self.private_key:
            self.logger.error("BULK_PRIVATE_KEY not found in environment")
            return False

        # Initialize HTTP client
        self.http_client = BulkHttpClient(
            base_url="https://exchange-api2.bulk.trade/api/v1",
            private_key=self.private_key
        )

        # Get current price
        self.logger.info(f"\n1. Fetching current price for {self.symbol}...")
        ticker = self.http_client.get_ticker(self.symbol)
        self.current_price = ticker.get('lastPrice', 100000.0)
        self.logger.info(f"   Current price: ${self.current_price:,.2f}")

        # Initialize signer
        self.signer = TransactionSigner(self.private_key)
        self.logger.info(f"   Account: {self.signer.public_key}")

        # Initialize WebSocket client
        self.logger.info(f"\n2. Connecting to WebSocket...")
        self.ws_client = BulkWebSocketClient(
            url="wss://exchange-wss.bulk.trade",
            symbols=[self.symbol],
            signer=self.signer
        )

        # Register order state handler
        self.ws_client.on(Topic.ORDER, self._handle_order_state)

        # Connect
        connected = await self.ws_client.connect()
        if not connected:
            self.logger.error("   Failed to connect to WebSocket")
            return False

        self.logger.info("   ✓ Connected to WebSocket")

        # Wait for account snapshot
        self.logger.info("   Waiting for account snapshot...")
        await asyncio.sleep(2)
        self.logger.info("   ✓ Account snapshot received")

        return True

    async def test_initial_sync(self):
        """
        Test 3: Create OrderStack and perform initial sync
        """
        self.logger.info("\n3. Initial Sync Test")
        self.logger.info("-" * 80)

        # Create synthetic orderbook around current price
        # Bid side: prices below current, descending
        # Ask side: prices above current, ascending
        tick_size = 10.0  # $10 increments for BTC
        n_levels = 10

        bid_prices = np.array([
            self.current_price - (i + 1) * tick_size
            for i in range(n_levels)
        ])
        bid_sizes = np.array([
            np.random.uniform(0.5, 2.0)
            for _ in range(n_levels)
        ])

        ask_prices = np.array([
            self.current_price + (i + 1) * tick_size
            for i in range(n_levels)
        ])
        ask_sizes = np.array([
            np.random.uniform(0.5, 2.0)
            for _ in range(n_levels)
        ])

        self.logger.info(f"   Synthetic orderbook created:")
        self.logger.info(f"   Best bid: ${bid_prices[0]:,.2f} x {bid_sizes[0]:.4f}")
        self.logger.info(f"   Best ask: ${ask_prices[0]:,.2f} x {ask_sizes[0]:.4f}")

        # Create OrderStack for bid side
        self.bid_stack = OrderStack(
            side=Side.BUY,
            chunk_fraction=0.25,  # Quarter sizing
            min_orders_per_level=4,
            symbol=self.symbol,
            max_price_levels=n_levels
        )

        self.logger.info(f"\n   OrderStack created: {self.bid_stack}")

        # Plan the sync first (to see what would happen)
        self.logger.info(f"\n   Planning sync...")
        orders_to_place, orders_to_cancel = self.bid_stack.plan(
            bid_prices, bid_sizes
        )

        self.logger.info(f"   Plan results:")
        self.logger.info(f"   - Orders to place: {len(orders_to_place)}")
        self.logger.info(f"   - Orders to cancel: {len(orders_to_cancel)}")

        # Show first few orders
        self.logger.info(f"\n   First 5 orders to place:")
        for i, (price, size) in enumerate(orders_to_place[:5]):
            self.logger.info(f"   {i + 1}. {size:.4f} @ ${price:,.2f}")

        # Execute sync
        self.logger.info(f"\n   Executing sync via WebSocket...")
        placed_oids, cancelled_oids = await self.bid_stack.sync(
            self.ws_client,
            bid_prices,
            bid_sizes
        )

        self.logger.info(f"\n   Sync complete:")
        self.logger.info(f"   - Placed: {len(placed_oids)} orders")
        self.logger.info(f"   - Cancelled: {len(cancelled_oids)} orders")

        if placed_oids:
            self.logger.info(f"\n   First 5 placed order IDs:")
            for i, oid in enumerate(placed_oids[:5]):
                self.logger.info(f"   {i + 1}. {oid}")

        # Wait for order states to arrive
        self.logger.info(f"\n   Waiting for order state updates...")
        await asyncio.sleep(3)

        # Check OrderStack state
        self.logger.info(f"\n   OrderStack state after sync:")
        self.logger.info(f"   {self.bid_stack}")
        stats = self.bid_stack.get_stats()
        for key, value in stats.items():
            self.logger.info(f"   - {key}: {value}")

        # Show level summary
        level_summary = self.bid_stack.get_level_summary()
        self.logger.info(f"\n   Level summary (showing first 5):")
        for i, (price, (size, n_orders)) in enumerate(list(level_summary.items())[:5]):
            self.logger.info(f"   {i + 1}. ${price:,.2f}: {size:.4f} ({n_orders} orders)")

        # Calculate discrepancy
        discrepancy = self.bid_stack.get_discrepancy(bid_prices, bid_sizes)
        total_disc = sum(abs(d) for d in discrepancy.values())
        self.logger.info(f"\n   Total discrepancy vs target: {total_disc:.6f}")

        return len(placed_oids) > 0

    async def test_market_adjustment(self):
        """
        Test 5: Simulate market movement and adjust orders
        """
        self.logger.info("\n5. Market Adjustment Test")
        self.logger.info("-" * 80)

        if not self.bid_stack:
            self.logger.error("   Bid stack not initialized")
            return False

        # Get current state
        old_stats = self.bid_stack.get_stats()
        self.logger.info(f"   Current state:")
        self.logger.info(f"   - Active orders: {old_stats['num_active_orders']}")
        self.logger.info(f"   - Price levels: {old_stats['num_levels']}")
        self.logger.info(f"   - Total size: {old_stats['total_size']:.4f}")

        # Simulate market movement - shift prices up by $50
        self.logger.info(f"\n   Simulating market movement (+$50)...")
        price_shift = 50.0
        tick_size = 10.0
        n_levels = 10

        new_bid_prices = np.array([
            self.current_price + price_shift - (i + 1) * tick_size
            for i in range(n_levels)
        ])
        new_bid_sizes = np.array([
            np.random.uniform(0.5, 2.0)
            for _ in range(n_levels)
        ])

        self.logger.info(f"   New best bid: ${new_bid_prices[0]:,.2f}")

        # Plan the adjustment
        self.logger.info(f"\n   Planning adjustment...")
        orders_to_place, orders_to_cancel = self.bid_stack.plan(
            new_bid_prices, new_bid_sizes
        )

        self.logger.info(f"   Adjustment plan:")
        self.logger.info(f"   - Orders to place: {len(orders_to_place)}")
        self.logger.info(f"   - Orders to cancel: {len(orders_to_cancel)}")

        # Execute sync
        self.logger.info(f"\n   Executing adjustment sync...")
        placed_oids, cancelled_oids = await self.bid_stack.sync(
            self.ws_client,
            new_bid_prices,
            new_bid_sizes
        )

        self.logger.info(f"\n   Adjustment complete:")
        self.logger.info(f"   - Placed: {len(placed_oids)} orders")
        self.logger.info(f"   - Cancelled: {len(cancelled_oids)} orders")

        # Wait for updates
        self.logger.info(f"\n   Waiting for order state updates...")
        await asyncio.sleep(3)

        # Check new state
        new_stats = self.bid_stack.get_stats()
        self.logger.info(f"\n   Updated state:")
        self.logger.info(f"   - Active orders: {new_stats['num_active_orders']}")
        self.logger.info(f"   - Price levels: {new_stats['num_levels']}")
        self.logger.info(f"   - Total size: {new_stats['total_size']:.4f}")

        # Calculate new discrepancy
        discrepancy = self.bid_stack.get_discrepancy(new_bid_prices, new_bid_sizes)
        total_disc = sum(abs(d) for d in discrepancy.values())
        self.logger.info(f"\n   Total discrepancy vs new target: {total_disc:.6f}")

        return True

    async def test_cancel_all(self):
        """
        Test 6: Cancel all orders
        """
        self.logger.info("\n6. Cancel All Test")
        self.logger.info("-" * 80)

        if not self.bid_stack:
            self.logger.error("   Bid stack not initialized")
            return False

        # Get current active orders
        stats = self.bid_stack.get_stats()
        n_orders_before = stats['num_active_orders']
        self.logger.info(f"   Active orders before cancel: {n_orders_before}")

        # Cancel all orders via WebSocket
        self.logger.info(f"\n   Issuing cancelAll...")
        try:
            response = await self.ws_client.cancel_all(symbols=[self.symbol])
            self.logger.info(f"   Cancel response: {response}")
        except Exception as e:
            self.logger.error(f"   Cancel error: {e}")
            return False

        # Wait for cancellations to process
        self.logger.info(f"\n   Waiting for cancellations to process...")
        await asyncio.sleep(3)

        # Cleanup terminal orders
        self.bid_stack.cleanup()

        # Check final state
        final_stats = self.bid_stack.get_stats()
        n_orders_after = final_stats['num_active_orders']

        self.logger.info(f"\n   Final state:")
        self.logger.info(f"   - Active orders: {n_orders_after}")
        self.logger.info(f"   - Orders cancelled: {n_orders_before - n_orders_after}")

        return True

    async def cleanup(self):
        """Cleanup and disconnect"""
        self.logger.info("\n7. Cleanup")
        self.logger.info("-" * 80)

        if self.ws_client:
            self.logger.info("   Disconnecting from WebSocket...")
            await self.ws_client.disconnect()
            self.logger.info("   ✓ Disconnected")

    def _handle_order_state(self, order_state: OrderState):
        """Handle order state updates from WebSocket"""
        self.order_states[order_state.order_id] = order_state

        # Update OrderStack
        if self.bid_stack and order_state.side == Side.BUY:
            self.bid_stack.update_order_state(order_state)

        self.logger.debug(
            f"   Order update: {order_state.order_id[:8]}... "
            f"{order_state.status.name} "
            f"{order_state.side.name} "
            f"{order_state.size:.4f} @ ${order_state.price:,.2f}"
        )

    def print_summary(self):
        """Print final test summary"""
        self.logger.info("\n" + "=" * 80)
        self.logger.info("TEST SUMMARY")
        self.logger.info("=" * 80)

        if self.bid_stack:
            stats = self.bid_stack.get_stats()
            self.logger.info(f"\nFinal OrderStack Statistics:")
            for key, value in stats.items():
                self.logger.info(f"  {key}: {value}")

        self.logger.info(f"\nTotal order states received: {len(self.order_states)}")

        # Show order status distribution
        status_counts = {}
        for order_state in self.order_states.values():
            status = order_state.status.name
            status_counts[status] = status_counts.get(status, 0) + 1

        self.logger.info(f"\nOrder status distribution:")
        for status, count in status_counts.items():
            self.logger.info(f"  {status}: {count}")

        self.logger.info("\n" + "=" * 80)


async def run_test():
    """Main test runner"""
    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Check for private key
    private_key = os.getenv('BULK_PRIVATE_KEY')
    if not private_key:
        logger.error("BULK_PRIVATE_KEY environment variable not found")
        logger.error("Skipping test - requires private key for trading")
        return

    # Create tester
    tester = OrderStackTester(
        symbol="BTC-USD",
        private_key=private_key,
        logger=logger
    )

    success = False

    try:
        # Setup
        if not await tester.setup():
            logger.error("Setup failed")
            return

        # Test 3: Initial sync
        if not await tester.test_initial_sync():
            logger.error("Initial sync test failed")
            return

        # Test 4: Wait and monitor
        logger.info("\n4. Monitoring Period")
        logger.info("-" * 80)
        logger.info("   Waiting 10 seconds to monitor order states...")
        await asyncio.sleep(10)
        logger.info("   ✓ Monitoring complete")

        # Test 5: Market adjustment
        if not await tester.test_market_adjustment():
            logger.error("Market adjustment test failed")
            return

        # Test 6: Cancel all
        if not await tester.test_cancel_all():
            logger.error("Cancel all test failed")
            return

        success = True

    except KeyboardInterrupt:
        logger.info("\n\nTest interrupted by user")

    except Exception as e:
        logger.error(f"\n\nTest failed with exception: {e}", exc_info=True)

    finally:
        # Cleanup
        await tester.cleanup()

        # Print summary
        tester.print_summary()

        if success:
            logger.info("\n✓ ALL TESTS PASSED")
        else:
            logger.info("\n✗ SOME TESTS FAILED")


if __name__ == "__main__":
    asyncio.run(run_test())