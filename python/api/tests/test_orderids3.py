#!/usr/bin/env python3
"""
Test Order ID Pre-computation

This test verifies that we can pre-compute order IDs using the hash function
and that they match the order IDs returned by the exchange.

Steps:
1. Connect to Bulk WebSocket
2. Create a limit order with a fixed nonce
3. Pre-compute the order ID using order.hash(pubkey)
4. Place the order
5. Verify the returned order ID matches our computed one
"""

import asyncio
import json
import logging
import os
import sys
import time
from pathlib import Path

from bulk import BulkHttpClient
from bulk.common import Side, TimeInForce, Topic
from bulk.common.signer import TransactionSigner
from bulk.messages.trade import LimitOrder, OrderResponse
from bulk.api import BulkWebSocketClient


# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

KEYS_FILE = "/tmp/bulk_test_keys.json"
TESTNET_WS_URL = "wss://exchange-wss.bulk.trade"
TESTNET_HTTP_URL = "https://exchange-api2.bulk.trade/api/v1"



async def get_funded_account() -> TransactionSigner:
    """
    Get funded account, creating and funding if not present
    """
    key = os.environ["BULK_TEST_KEY"]
    return TransactionSigner(key)

class OrderIDTest:
    """Test harness for order ID pre-computation"""

    def __init__(self, signer: TransactionSigner):
        self.signer = signer
        self.client = BulkWebSocketClient(
            url="wss://exchange-wss.bulk.trade",
            symbols=["BTC-USD"],
            signer=self.signer,
            logger=logger
        )

        # Test results
        self.test_passed = False
        self.computed_order_id = None
        self.returned_order_id = None
        self.order_response = None

    async def run_test(self):
        """Run the order ID test"""
        logger.info("=" * 80)
        logger.info("ORDER ID PRE-COMPUTATION TEST")
        logger.info("=" * 80)

        try:
            # Step 1: Connect to WebSocket
            logger.info("\n[Step 1] Connecting to Bulk WebSocket...")
            connected = await self.client.connect()
            if not connected:
                logger.error("Failed to connect to WebSocket")
                return False

            logger.info("✓ Connected successfully")
            logger.info(f"  Public key: {self.signer.public_key}")

            # Wait for account snapshot
            await asyncio.sleep(2)

            # Step 2: Create limit order with fixed nonce
            logger.info("\n[Step 2] Creating limit order with fixed nonce...")

            order1b = LimitOrder(
                symbol="BTC-USD",
                side=Side.BUY,
                price=95104.75,
                size=0.6704 - 1e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )
            order2b = LimitOrder(
                symbol="BTC-USD",
                side=Side.BUY,
                price=95104.75,
                size=0.6704 - 2e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )
            order3b = LimitOrder(
                symbol="BTC-USD",
                side=Side.BUY,
                price=95104.75,
                size=0.6704 - 3e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )
            order4b = LimitOrder(
                symbol="BTC-USD",
                side=Side.BUY,
                price=95100.75,
                size=0.6704 - 1e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )
            order5b = LimitOrder(
                symbol="BTC-USD",
                side=Side.BUY,
                price=95100.75,
                size=0.6704 - 2e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )

            order1s = LimitOrder(
                symbol="BTC-USD",
                side=Side.SELL,
                price=95214.75,
                size=0.6704 + 1e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )
            order2s = LimitOrder(
                symbol="BTC-USD",
                side=Side.SELL,
                price=95214.75,
                size=0.6704 + 2e-8,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )

            # Step 3: Place the orders
            logger.info("\n[Step 4] Placing orders on exchange...")

            orders = [order1b, order2b, order3b, order4b, order5b, order1s, order2s]
            responses = await self.client.place_orders(orders)

            self.test_passed = True
            for i in range(len(responses)):
                oid = orders[i].hash(pubkey=self.signer.public_key, nonce=orders[i].nonce)
                oid_response = responses[i].order_id
                if oid != oid_response:
                    logger.error(f"Response ID {oid_response} does not match order oid {oid_response}, for: {orders[i]}")
                    self.test_passed = False

            return self.test_passed

        except Exception as e:
            logger.error(f"Test error: {e}", exc_info=True)
            return False

        finally:
            # Disconnect
            logger.info("\n[Cleanup] Disconnecting...")
            await self.client.disconnect()
            logger.info("✓ Disconnected")

    def print_summary(self):
        """Print test summary"""
        logger.info("\n" + "=" * 80)
        logger.info("TEST SUMMARY")
        logger.info("=" * 80)

        if self.test_passed:
            logger.info("✓ TEST PASSED")
            logger.info(f"  Computed order IDs match returned order ID")
        else:
            logger.error("✗ TEST FAILED")

        logger.info("=" * 80)


async def main():
    """Main test entry point"""

    # Run test
    signer = await get_funded_account()
    test = OrderIDTest(signer)
    success = await test.run_test()
    test.print_summary()

    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    asyncio.run(main())