#!/use/bin/env python3
#
# Random (OU-Process) Market Maker for Performance Testing
#
# Generates a synthetic order book around an Ornstein-Uhlenbeck mid price.
# Each cycle:
#   1. Cancel all existing orders
#   2. Generate a full book (bids + asks) with configurable depth and orders per level
#   3. Send as a single batch
#
# Features:
# - OU process mid price with mean-reversion (kappa) and volatility (sigma)
# - Size per level increases with depth (more liquidity further from mid)
# - Configurable spread, depth, and orders per level
# - No order tracking needed (cancel-all at start of each cycle)
#

import argparse
import asyncio
import logging
import math
import os
import signal
import sys
import time
from pathlib import Path
from typing import Optional, List

import numpy as np

project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from bulk import BulkWebSocketClient, SimulatedWebSocketClient
from bulk import Topic as BulkTopic
from bulk.common import Side, TimeInForce
from bulk.common.signer import TransactionSigner
from bulk.messages import LimitOrder, CancelAll
from bulk.messages import OrderState, Fill, PositionUpdate, OraclePrices

from mm.rmm.common import RMMConfig


class OUProcess:
    """
    Ornstein-Uhlenbeck process for mid price simulation.

    dX = kappa * (mu - X) * dt + sigma * dW

    where:
        mu    = long-run mean (config.price)
        kappa = mean-reversion speed
        sigma = volatility
    """

    def __init__(self, p0: float, kappa: float, sigma: float, mu: float = 0.0):
        self.p0 = p0
        self.kappa = kappa
        self.sigma = sigma
        self.mu = mu
        self.cumr = mu
        self.price = p0

    def step(self, dt: float) -> float:
        """Advance the process by dt seconds, return new value."""
        dw = np.random.normal(0, math.sqrt(dt))
        self.cumr += self.kappa * (self.mu - self.cumr) * dt + self.sigma * dw

        # convert to price
        self.price = self.p0 * np.exp(self.cumr * 1e-4)
        return self.price


class RandomMarketMaker:
    """
    Performance-testing market maker that posts a full synthetic book each cycle.

    Architecture:
        - OUProcess generates a drifting mid price
        - Each tick: cancel_all + place (depth * 2 * orders_per_level) limit orders
        - Size at each level = base_size * (1 + level_index * growth_factor)
    """

    def __init__(self, config: RMMConfig):
        self.config = config
        self.logger = logging.getLogger(__name__)

        # Clients / signer
        self.bulk: Optional[BulkWebSocketClient] = None
        self.signer: Optional[TransactionSigner] = None

        # OU process
        self.ou = OUProcess(
            p0=config.price,
            kappa=config.kappa,
            sigma=config.sigma,
        )

        # State
        self.running = False
        self.total_orders = 0
        self.tick_count = 0

    # ==================== LIFECYCLE ====================

    async def initialize(self) -> bool:
        """Initialize signer and exchange connection."""
        self.logger.info("=" * 70)
        self.logger.info("RANDOM MARKET MAKER — INITIALIZATION")
        self.logger.info("=" * 70)

        try:
            # Signer
            self.signer = TransactionSigner(self.config.private_key)
            self.logger.info(f"Account: {self.signer.public_key}")

            # Bulk connection
            self.logger.info(
                f"Connecting to Bulk ({self.config.bulk_symbol()}) "
                f"on {self.config.bulk_ws_url} ..."
            )

            if not self.config.dryrun:
                self.bulk = BulkWebSocketClient(
                    url=self.config.bulk_ws_url,
                    symbols=[self.config.bulk_symbol()],
                    signer=self.signer,
                    handlers={
                        BulkTopic.ORDER: self._handle_order_state,
                        BulkTopic.FILL: self._handle_fill,
                        BulkTopic.POSITION: self._handle_position_update,
                    },
                )
            else:
                self.bulk = SimulatedWebSocketClient(
                    symbols=[self.config.bulk_symbol()],
                    signer=self.signer,
                    handlers={
                        BulkTopic.ORDER: self._handle_order_state,
                        BulkTopic.FILL: self._handle_fill,
                        BulkTopic.POSITION: self._handle_position_update,
                    },
                )

            connected = await self.bulk.connect()
            if not connected:
                self.logger.error("Failed to connect to Bulk")
                return False

            self.logger.info("Connected to Bulk")
            await asyncio.sleep(2)  # wait for account snapshot

            # Cancel any stale orders
            await self.bulk.cancel_all(symbols=[self.config.bulk_symbol()])
            self.logger.info("Cancelled any pre-existing orders")

            self.logger.info("=" * 70)
            self.logger.info("INITIALIZATION COMPLETE")
            self.logger.info(
                f"  mid={self.config.price}  kappa={self.config.kappa}  "
                f"sigma={self.config.sigma}  spread={self.config.spread}"
            )
            self.logger.info(
                f"  depth={self.config.depth}  orders/level={self.config.order_per_level}  "
                f"freq={self.config.frequency}s"
            )
            self.logger.info("=" * 70)
            return True

        except Exception as e:
            self.logger.error(f"Initialization failed: {e}", exc_info=True)
            return False

    async def run(self):
        """Main loop: evolve price, rebuild book each tick."""
        self.running = True
        self.tick_count = 0
        self.logger.info("Starting random MM loop …")

        try:
            while self.running:
                t0 = time.time()
                await self._tick()
                elapsed = time.time() - t0
                sleep_time = max(0.0, self.config.frequency - elapsed)
                await asyncio.sleep(sleep_time)

        except asyncio.CancelledError:
            self.logger.info("Loop cancelled")
        except Exception as e:
            self.logger.error(f"Loop error: {e}", exc_info=True)
        finally:
            await self.shutdown()

    async def shutdown(self):
        """Cancel everything and disconnect."""
        self.logger.info("Shutting down …")
        self.running = False

        if self.bulk and self.bulk.is_connected:
            try:
                await self.bulk.cancel_all(symbols=[self.config.bulk_symbol()])
            except Exception as e:
                self.logger.error(f"Error cancelling on shutdown: {e}")
            await self.bulk.disconnect()

        self.logger.info(
            f"Done. Ticks={self.tick_count}  TotalOrders={self.total_orders}"
        )

    # ==================== CORE TICK ====================

    async def _tick(self):
        """
        Single cycle:
        - evolve mid
        - oracle price updaye
        - cancel all
        - place full book.
        """
        timestamp = int(time.time_ns())
        nonce = int(timestamp / 1000)
        self.tick_count += 1

        # 1. Evolve mid price
        mid = self.ou.step(self.config.frequency)

        # 2. periodically send oracle updates
        tasks = []
        prelude = self.tick_count <= 2000
        if prelude or self.tick_count % 50 == 0:
            oracle = OraclePrices(timestamp=timestamp, prices={self.config.coin: mid}, nonce=nonce)
            tasks.append(self.bulk.update_oracle(oracle, nonce=nonce))
            self.logger.info(f"oracle tick: {self.tick_count}")

        # 3. Build order list
        if not prelude:
            orders = self._build_book(mid, nonce)
            cancel = CancelAll(symbols=[], nonce=nonce)
            actions: list = [cancel] + orders
            tasks.append(self.bulk.place_orders(actions, nonce=nonce))

            if self.tick_count % 100 == 0:
                self.logger.info(f"book update tick: {self.tick_count}, {len(orders)} orders")

        # now evaluate pending tx
        try:
            results = await asyncio.gather(*tasks, return_exceptions=True)
            if not prelude:
                responses = results[-1]
                n_err = sum(1 for r in responses if r.is_error())
                self.total_orders += len(orders)

                if n_err > 0:
                    self.logger.warning(f"Cycle {self.tick_count}: {n_err}/{len(responses)} errors")

                if self.tick_count % 1000 == 0:
                    self.logger.info(f"Cycle {self.tick_count}: mid=${mid:,.2f}  "f"orders={len(orders)}  total={self.total_orders}")
        except Exception as e:
            self.logger.error(f"Tick execution error: {e}")


    # ==================== BOOK GENERATION ====================

    def _build_book(self, mid: float, nonce: int) -> List[LimitOrder]:
        """
        Generate a full symmetric order book around *mid*.

        - Spread between best bid / best ask = config.spread
        - Level spacing = spread (uniform tick)
        - Size grows linearly with distance from mid:
              level_size = base_size * (1 + level_index * 0.5)
          Each order at that level = level_size / order_per_level
        """
        cfg = self.config
        half_spread = cfg.spread / 2.0
        # smallest level size (in coins)
        base_size = cfg.size
        # linear growth factor per level
        growth = 0.01

        orders: List[LimitOrder] = []

        for level_idx in range(cfg.depth):
            # Price offset from mid
            offset = half_spread + level_idx * cfg.spread

            bid_price = round(mid - offset, 2)
            ask_price = round(mid + offset, 2)

            if bid_price <= 0:
                continue

            # Size increases with depth
            level_size = base_size * (1.0 + level_idx * growth)
            order_size = round(level_size / cfg.order_per_level, 6)

            if order_size <= 0:
                continue

            for i in range(cfg.order_per_level):
                size = order_size + i * 1e-5
                orders.append(LimitOrder(
                    symbol=cfg.bulk_symbol(),
                    side=Side.BUY,
                    price=bid_price,
                    size=size,
                    reduce_only=False,
                    time_in_force=TimeInForce.GTC,
                    pubkey=self.signer.public_key,
                    nonce=nonce,
                ))
                orders.append(LimitOrder(
                    symbol=cfg.bulk_symbol(),
                    side=Side.SELL,
                    price=ask_price,
                    size=size,
                    reduce_only=False,
                    time_in_force=TimeInForce.GTC,
                    pubkey=self.signer.public_key,
                    nonce=nonce,
                ))

        return orders

    # ==================== EVENT HANDLERS (minimal) ====================

    def _handle_order_state(self, order_state: OrderState):
        pass  # no tracking needed

    def _handle_fill(self, fill: Fill):
        self.logger.debug(
            f"FILL: {fill.side.name} {fill.size:.6f} @ ${fill.price:,.2f}"
        )

    def _handle_position_update(self, position: PositionUpdate):
        if abs(position.size) > 1e-6:
            self.logger.debug(
                f"POS: {position.size:+.6f}  "
                f"notional=${position.notional:,.2f}  "
                f"uPnL=${position.unrealized_pnl:+,.2f}"
            )


# ==================== ENTRY POINT ====================

async def main():
    parser = argparse.ArgumentParser(
        description="OU-Process Random Market Maker (performance testing)",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--config", type=str, required=True, help="JSON5 config file")
    parser.add_argument(
        "--log-level", type=str, default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
    )
    args = parser.parse_args()

    config = RMMConfig.load(args.config)

    logging.basicConfig(
        level=getattr(logging, args.log_level),
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    )
    logger = logging.getLogger(__name__)

    private_key = os.getenv("BULK_PRIVATE_KEY")
    if not private_key:
        logger.error("Set BULK_PRIVATE_KEY env var")
        return 1
    config.private_key = private_key

    mm = RandomMarketMaker(config)

    def signal_handler(signum, frame):
        logger.info(f"Signal {signum} received, stopping …")
        mm.running = False

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    if not await mm.initialize():
        return 1

    try:
        await mm.run()
        return 0
    except Exception as e:
        logger.error(f"Fatal: {e}", exc_info=True)
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))