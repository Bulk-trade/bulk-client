"""
Binance Order Book Replication Market Maker

Replicates Binance USDT-M perpetual futures order book on Bulk Labs exchange.

Features:
- Configurable via command-line arguments
- Dual-sided order management (bid and ask stacks)
- Automatic inventory management with position closure
- Binance WebSocket monitoring and reconnection
- Periodic synchronization with configurable frequency
- Clean shutdown handling
"""

import argparse
import asyncio
import logging
import os
import sys
import signal
import time
from pathlib import Path
from typing import Optional

import numpy as np

from messages import CancelAll

# Add parent directory to path for imports
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from bulk import BulkWebSocketClient
from bulk import Topic as BulkTopic
from bulk.common import Side, TimeInForce, SequenceCounter
from bulk.common.inventory import Inventory
from bulk.common.signer import TransactionSigner
from bulk.messages import MarketOrder, LimitOrder, CancelOrder
from bulk.messages import OrderState, Fill, PositionUpdate
from bulk.messages import L2Delta

from mm.bmm.adaptors import BinanceFuturesWebSocketClient, Topic as BinanceTopic
from mm.bmm.common import OrderStack
from mm.bmm.common import MMConfig

class BinanceMarketMaker:
    """
    Market maker that replicates Binance order book on Bulk Labs

    Architecture:
    - Binance WebSocket: Source of truth for order book
    - Bulk WebSocket: Execution venue
    - OrderStack (bid/ask): Order management and synchronization
    - Inventory: Position tracking and risk management
    """

    def __init__(self, config: MMConfig):
        self.config = config
        self.logger = logging.getLogger(__name__)

        # Clients
        self.binance: Optional[BinanceFuturesWebSocketClient] = None
        self.bulk: Optional[BulkWebSocketClient] = None
        self.signer: Optional[TransactionSigner] = None

        # Order management
        self.seqno = SequenceCounter()
        self.bid_stack: Optional[OrderStack] = None
        self.ask_stack: Optional[OrderStack] = None

        # Position tracking
        self.current_position_size: float = 0.0
        self.current_position_notional: float = 0.0

        # State tracking
        self.running = False
        self.last_binance_update = time.time()
        self.last_sync_time = 0.0
        self.sync_count = 0
        self.total_orders_placed = 0
        self.total_orders_cancelled = 0

        # Tasks
        self.sync_task: Optional[asyncio.Task] = None
        self.monitor_task: Optional[asyncio.Task] = None
        self.binance_monitor_task: Optional[asyncio.Task] = None

    async def initialize(self) -> bool:
        """Initialize all components"""
        self.logger.info("=" * 80)
        self.logger.info("BINANCE MARKET MAKER - INITIALIZATION")
        self.logger.info("=" * 80)

        try:
            # Initialize signer
            self.logger.info("\n1. Initializing transaction signer...")
            self.signer = TransactionSigner(self.config.private_key)
            self.logger.info(f"   Account: {self.signer.public_key}")

            # Initialize Binance client
            self.logger.info(f"\n2. Connecting to Binance ({self.config.binance_symbol()})...")
            self.binance = BinanceFuturesWebSocketClient(
                symbols=[self.config.binance_symbol()],
                snapshot_limit=1000,
                enable_bbo=False,
                handlers={
                    BinanceTopic.L2_DELTA: self._handle_binance_delta,
                }
            )

            connected = await self.binance.connect()
            if not connected:
                self.logger.error("   Failed to connect to Binance")
                return False
            self.logger.info("   ✓ Connected to Binance")

            # Wait for initial book data
            self.logger.info("   Waiting for Binance order book...")
            await asyncio.sleep(2)

            # Verify book is populated
            book = self.binance.get_book(self.config.binance_symbol())
            if not book or book.get_mid_price() is None:
                self.logger.error("   Failed to initialize Binance order book")
                return False

            mid = book.get_mid_price()
            self.logger.info(f"   ✓ Binance book ready (mid: ${mid:,.2f})")

            # Initialize order stacks
            self.logger.info(f"\n6. Initializing order stacks...")
            self.bid_stack = OrderStack(
                symbol=self.config.bulk_symbol(),
                side=Side.BUY,
                seqno=self.seqno,
                chunk_size=self.config.chunk_size,
                max_orders=self.config.max_new_orders,
                max_price_levels=self.config.max_price_levels,
                signer=self.signer,
            )

            self.ask_stack = OrderStack(
                symbol=self.config.bulk_symbol(),
                side=Side.SELL,
                seqno=self.seqno,
                chunk_size=self.config.chunk_size,
                max_orders=self.config.max_new_orders,
                max_price_levels=self.config.max_price_levels,
                signer=self.signer,
            )

            self.logger.info("   ✓ Order stacks initialized")
            self.logger.info(f"      Bid stack: {self.bid_stack}")
            self.logger.info(f"      Ask stack: {self.ask_stack}")

            # Initialize Bulk client
            self.logger.info(f"\n3. Connecting to Bulk for ({self.config.bulk_symbol()}), on: {self.config.bulk_ws_url}...")
            self.bulk = BulkWebSocketClient(
                url=self.config.bulk_ws_url,
                symbols=[self.config.bulk_symbol()],
                signer=self.signer,
                handlers={
                    BulkTopic.ORDER: self._handle_order_state,
                    BulkTopic.FILL: self._handle_fill,
                    BulkTopic.POSITION: self._handle_position_update,
                }
            )

            connected = await self.bulk.connect()
            if not connected:
                self.logger.error("   Failed to connect to Bulk")
                return False
            self.logger.info("   ✓ Connected to Bulk")

            # Wait for account snapshot
            self.logger.info("   Waiting for account snapshot...")
            await asyncio.sleep(2)
            self.logger.info("   ✓ Account snapshot received")

            # Cancel all existing orders
            self.logger.info("\n5. Cancelling all existing orders...")
            try:
                result = await self.bulk.cancel_all(symbols=[self.config.bulk_symbol()])
                self.logger.info(f"   ✓ Cancelled all orders: {result}")
            except Exception as e:
                self.logger.warning(f"   Cancel all failed (might be no orders): {e}")

            # Close any inventory
            position = self.bulk.inventory.quantity_for(self.config.bulk_symbol())
            side = Side.SELL if position < 0.0 else Side.BUY

            self.logger.info(f"\n6. Closing all inventory, pos: {position}...")
            if position != 0:
                mid = book.get_mid_price()
                if side == Side.BUY:
                    maxpx = mid * (1 + self.config.inventory_close_depth * 1e-4)
                else:
                    maxpx = mid * (1 - self.config.inventory_close_depth * 1e-4)

                try:
                    result = await self.bulk.place_limit_order(
                        symbol = self.config.bulk_symbol(),
                        side = side,
                        size = abs(position),
                        price = maxpx,
                        reduce_only=True,
                        time_in_force=TimeInForce.IOC,
                    )
                    self.logger.info(f"   ✓ Close position status: {result}")
                except Exception as e:
                    self.logger.warning(f"   Closing position failed: {e}")

            self.logger.info("=" * 80)
            self.logger.info("INITIALIZATION COMPLETE - READY TO RUN")
            self.logger.info("=" * 80)
            return True

        except Exception as e:
            self.logger.error(f"Initialization failed: {e}", exc_info=True)
            return False

    async def run(self):
        """Main market maker loop"""
        self.running = True
        self.logger.info("\nStarting market maker...")

        # Start background tasks
        self.sync_task = asyncio.create_task(self._sync_loop())
        self.monitor_task = asyncio.create_task(self._monitor_loop())
        self.binance_monitor_task = asyncio.create_task(self._binance_monitor_loop())

        try:
            # Run until stopped
            while self.running:
                await asyncio.sleep(1)

        except asyncio.CancelledError:
            self.logger.info("Market maker cancelled")
        except Exception as e:
            self.logger.error(f"Market maker error: {e}", exc_info=True)
        finally:
            await self.shutdown()

    async def shutdown(self):
        """Clean shutdown"""
        self.logger.info("=" * 80)
        self.logger.info("SHUTTING DOWN MARKET MAKER")
        self.logger.info("=" * 80)

        self.running = False

        # Cancel background tasks
        for task in [self.sync_task, self.monitor_task, self.binance_monitor_task]:
            if task:
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass

        # Cancel all orders
        if self.bulk and self.bulk.is_connected:
            self.logger.info("\nCancelling all orders...")
            try:
                await self.bulk.cancel_all(symbols=[self.config.bulk_symbol()])
                self.logger.info("✓ All orders cancelled")
            except Exception as e:
                self.logger.error(f"Error cancelling orders: {e}")

        # Disconnect clients
        if self.binance:
            self.logger.info("Disconnecting from Binance...")
            await self.binance.disconnect()
            self.logger.info("✓ Disconnected from Binance")

        if self.bulk:
            self.logger.info("Disconnecting from Bulk...")
            await self.bulk.disconnect()
            self.logger.info("✓ Disconnected from Bulk")

        # Print final statistics
        self._print_statistics()

        self.logger.info("\n" + "=" * 80)
        self.logger.info("SHUTDOWN COMPLETE")
        self.logger.info("=" * 80)

    # ==================== SYNC LOOP ====================

    async def _sync_loop(self):
        """Periodic synchronization loop"""
        self.logger.info(f"Starting sync loop (interval: {self.config.sync_interval}s)")

        while self.running:
            try:
                await asyncio.sleep(self.config.sync_interval)
                await self._sync_orders()

            except asyncio.CancelledError:
                break
            except Exception as e:
                self.logger.error(f"Sync loop error: {e}", exc_info=True)
                await asyncio.sleep(1)

    async def _sync_orders(self):
        """Synchronize orders with Binance book"""
        start_time = time.time()

        # Get Binance book
        binance_book = self.binance.get_book(self.config.binance_symbol())
        if not binance_book:
            self.logger.warning("Binance book not available")
            return

        # Get bid and ask arrays
        bid_prices, bid_sizes = binance_book.aggregate_dual(
            Side.BUY, self.config.fine_tick, self.config.coarse_tick)
        ask_prices, ask_sizes = binance_book.aggregate_dual(
            Side.SELL, self.config.fine_tick, self.config.coarse_tick)

        nonce = time.time_ns()
        clears = []

        # reset action count
        self.seqno.reset()

        # if the number of outstanding orders exceeds maximum, clear
        orders = self.bulk.get_order_map()
        if len(orders) > self.config.max_live_orders:
            self.logger.info(f"clearing an excess of orders: {len(orders)}")
            self.bid_stack.clear()
            self.ask_stack.clear()
            cancel = CancelAll(
                symbols=[self.config.bulk_symbol()],
                seqno=self.seqno.next(),
            )
            clears.append(cancel)

        # Sync bid side
        bid_placed, bid_cancelled, bidx_added, bidx_deleted = self.bid_stack.plan(
            bid_prices,
            bid_sizes,
            self.config.max_depth,
            nonce=nonce
        )

        # Sync ask side
        ask_placed, ask_cancelled, askx_added, askx_deleted = self.ask_stack.plan(
            ask_prices,
            ask_sizes,
            self.config.max_depth,
            nonce=nonce
        )

        # Execute
        self.logger.info(
            f"updating {self.config.bulk_symbol()} book with "
            f"{len(bid_placed) + len(ask_placed)} new orders "
            f"+ {len(bid_cancelled)+len(ask_cancelled)} cancels, "
            f"open orders: {len(self.bulk.get_order_map())}")

        actions = sorted([*clears, *bid_cancelled, *ask_cancelled, *bid_placed, *ask_placed], key=lambda a: a.seqno)

        if len(actions) > 0:
            responses = await self.bulk.place_orders(actions, nonce=nonce)
            for i, response in enumerate(responses):
                if response.is_error():
                    self.logger.error(f"Error executing order: {actions[i]}: {response}")
                    # handle termination that was not notified
                    if response.order_id and isinstance(actions[i], CancelOrder):
                        order_id = response.order_id
                        side = actions[i].side

                        self.logger.debug(f"handling cancel failure for: {order_id}")
                        stack = self.bid_stack if side == Side.BUY else self.ask_stack
                        stack.terminated(order_id)


        # Update statistics
        self.sync_count += 1
        self.total_orders_placed += len(bid_placed) + len(ask_placed)
        self.total_orders_cancelled += len(bid_cancelled) + len(ask_cancelled)
        self.last_sync_time = time.time()

        elapsed = (time.time() - start_time) * 1000

        if bid_placed or bid_cancelled or ask_placed or ask_cancelled:
            self.logger.info(
                f"Sync #{self.sync_count}: "
                f"Placed={len(bid_placed) + len(ask_placed)} "
                f"Cancelled={len(bid_cancelled) + len(ask_cancelled)} "
                f"Time={elapsed:.1f}ms, "
                f"BBO: {bid_sizes[0]} @ {bid_prices[0]} / {ask_sizes[0]} @ {ask_prices[0]}"
            )

    # ==================== MONITORING ====================

    async def _monitor_loop(self):
        """Periodic monitoring and risk management"""
        self.logger.info(
            f"Starting monitor loop (interval: {self.config.monitor_interval}s)"
        )

        while self.running:
            try:
                await asyncio.sleep(self.config.monitor_interval)
                await self._check_inventory()

            except asyncio.CancelledError:
                break
            except Exception as e:
                self.logger.error(f"Monitor loop error: {e}", exc_info=True)
                await asyncio.sleep(1)

    async def _check_inventory(self):
        """Check inventory and close position if needed"""
        # Get current position notional
        notional = abs(self.current_position_notional)

        if notional > self.config.max_inventory_notional:
            self.logger.warning(
                f"INVENTORY LIMIT EXCEEDED: ${notional:,.2f} > "
                f"${self.config.max_inventory_notional:,.2f}"
            )

            # Calculate size to close
            size_to_close = abs(self.current_position_size) * self.config.inventory_close_fraction
            side = Side.SELL if self.current_position_size > 0 else Side.BUY

            try:
                # Place market order to reduce position
                market_order = MarketOrder(
                    symbol=self.config.bulk_symbol(),
                    side=side,
                    size=size_to_close,
                    reduce_only=True,
                    nonce=int(time.time_ns() / 1000),
                )

                self.logger.warning(f"Closing position: {side} {size_to_close:.4f} @ MARKET")
                response = await self.bulk.place_orders([market_order])
                self.logger.info(f"Position close response: {response}")

            except Exception as e:
                self.logger.error(f"Error closing position: {e}", exc_info=True)

    async def _binance_monitor_loop(self):
        """Monitor Binance connection health"""
        self.logger.info(
            f"Starting Binance monitor (timeout: {self.config.binance_timeout}s)"
        )

        while self.running:
            try:
                await asyncio.sleep(5)  # Check every 5 seconds

                time_since_update = time.time() - self.last_binance_update

                if time_since_update > self.config.binance_timeout:
                    self.logger.error(
                        f"Binance timeout: {time_since_update:.1f}s since last update"
                    )
                    self.logger.info("Reconnecting to Binance...")

                    try:
                        await self.binance.disconnect()
                        await asyncio.sleep(2)
                        connected = await self.binance.connect()

                        if connected:
                            self.logger.info("✓ Reconnected to Binance")
                            self.last_binance_update = time.time()
                        else:
                            self.logger.error("Failed to reconnect to Binance")

                    except Exception as e:
                        self.logger.error(f"Reconnection error: {e}", exc_info=True)

            except asyncio.CancelledError:
                break
            except Exception as e:
                self.logger.error(f"Binance monitor error: {e}", exc_info=True)

    # ==================== EVENT HANDLERS ====================

    def _handle_binance_delta(self, binance, delta: L2Delta):
        """Handle Binance order book delta"""
        self.last_binance_update = time.time()

    def _handle_order_state(self, order_state: OrderState):
        """Handle order state update from Bulk"""
        # Update appropriate stack
        if order_state.side == Side.BUY:
            self.bid_stack.update_order_state(order_state)
        else:
            self.ask_stack.update_order_state(order_state)

        if "STATE" in self.config.to_log:
            self.logger.info(
                f"Order {order_state.order_id} "
                f"{order_state.status.name} "
                f"{order_state.side.name} "
                f"{order_state.size:.4f} @ ${order_state.price:,.2f}"
            )
        else:
            pass

    def _handle_fill(self, fill: Fill):
        """Handle fill (trade execution)"""
        if "FILL" in self.config.to_log:
            self.logger.info(
                f"FILL: {fill.symbol} "
                f"{fill.side.name} {fill.size:.4f} @ ${fill.price:,.2f} "
                f"{'MAKER' if fill.is_maker else 'TAKER'}"
            )
        else:
            pass

    def _handle_position_update(self, position: PositionUpdate):
        """Handle position update"""
        self.current_position_size = position.size
        self.current_position_notional = position.notional

        # Only log if we have a position
        if abs(position.size) > 0.001 and "POSITION" in self.config.to_log:
            self.logger.info(
                f"POSITION: {position.symbol} "
                f"Size={position.size:+.4f} "
                f"Notional=${position.notional:,.2f} "
                f"PnL=${position.unrealized_pnl:+,.2f}"
            )

    # ==================== STATISTICS ====================

    def _print_statistics(self):
        """Print final statistics"""
        self.logger.info("\nFINAL STATISTICS:")
        self.logger.info("-" * 80)

        # Sync statistics
        self.logger.info(f"Total syncs: {self.sync_count}")
        self.logger.info(f"Total orders placed: {self.total_orders_placed}")
        self.logger.info(f"Total orders cancelled: {self.total_orders_cancelled}")

        # Order stack statistics
        if self.bid_stack:
            bid_stats = self.bid_stack.get_stats()
            self.logger.info(f"\nBid Stack:")
            for key, value in bid_stats.items():
                self.logger.info(f"  {key}: {value}")

        if self.ask_stack:
            ask_stats = self.ask_stack.get_stats()
            self.logger.info(f"\nAsk Stack:")
            for key, value in ask_stats.items():
                self.logger.info(f"  {key}: {value}")

        # Position statistics
        self.logger.info(f"\nFinal Position:")
        self.logger.info(f"  Size: {self.current_position_size:+.4f}")
        self.logger.info(f"  Notional: ${self.current_position_notional:,.2f}")

        # Inventory P&L
        binance_book = self.binance.get_book(self.config.binance_symbol())
        if binance_book:
            mid_price = binance_book.get_mid_price()
            if mid_price:
                prices = {self.config.symbol: mid_price}
                pnl = self.inventory.pv(prices)
                self.logger.info(f"\nInventory P&L:")
                self.logger.info(f"  Realized: ${pnl.realized:+,.2f}")
                self.logger.info(f"  Unrealized: ${pnl.unrealized:+,.2f}")
                self.logger.info(f"  Net: ${pnl.net:+,.2f}")


def parse_args():
    """Parse command-line arguments"""
    parser = argparse.ArgumentParser(
        description="Binance Order Book Replication Market Maker",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter
    )

    # Symbol configuration
    parser.add_argument(
        "--config",
        type=str,
        required=True,
        help="configuration file"
    )

    # Logging
    parser.add_argument(
        "--log-level",
        type=str,
        default=None,
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        help="Logging level"
    )
    return parser.parse_args()


async def main():
    """Main entry point"""
    # Parse arguments
    args = parse_args()

    config = MMConfig.load(args.config)
    if args.log_level:
        config.log_level = args.log_level

    # Setup logging
    logging.basicConfig(
        level=getattr(logging, config.log_level),
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Get private key
    private_key = os.getenv('BULK_PRIVATE_KEY')
    if not private_key:
        logger.error("Private key required: BULK_PRIVATE_KEY env var should be set")
        return 1
    else:
        config.private_key = private_key

    # Create market maker
    mm = BinanceMarketMaker(config)

    # Setup signal handlers for clean shutdown
    loop = asyncio.get_event_loop()
    main_task = None

    def _handle_shutdown():
        logger.info("\nReceived shutdown signal, exiting immediately...")
        if main_task and not main_task.done():
            main_task.cancel()

    loop.add_signal_handler(signal.SIGINT, _handle_shutdown)
    loop.add_signal_handler(signal.SIGTERM, _handle_shutdown)

    # Initialize
    if not await mm.initialize():
        logger.error("Initialization failed")
        return 1

    # Run market maker
    try:
        main_task = asyncio.current_task()
        await mm.run()
        return 0
    except asyncio.CancelledError:
        logger.info("Shutting down...")
        return 0
    except Exception as e:
        logger.error(f"Fatal error: {e}", exc_info=True)
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))