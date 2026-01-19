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

            order = LimitOrder(
                symbol="BTC-USD",
                side=Side.SELL,
                price=95204.75,
                size=0.6704 + 1e-6,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                nonce=1768654732092639
            )

            logger.info(f"✓ Order created:")
            logger.info(f"  Symbol: {order.symbol}")
            logger.info(f"  Side: {order.side}")
            logger.info(f"  Price: ${order.price:,.2f}")
            logger.info(f"  Size: {order.size}")
            logger.info(f"  Nonce: {order.nonce}")
            logger.info(f"  Time in Force: {order.time_in_force}")

            # Step 3: Pre-compute order ID
            logger.info("\n[Step 3] Pre-computing order ID...")

            self.computed_order_id = order.hash(self.signer.public_key, order.nonce)
            logger.info(f"✓ Computed order ID: {self.computed_order_id}")

            # Step 4: Place the order
            logger.info("\n[Step 4] Placing order on exchange...")

            responses = await self.client.place_orders([order])
            response = responses[0]

            self.order_response = response

            # Check for errors
            if response.is_error():
                logger.error(f"✗ Order placement failed:")
                logger.error(f"  Status: {response.status}")
                logger.error(f"  Message: {response.message}")
                logger.error(f"  Meta: {response.meta}")
                return False

            self.returned_order_id = response.order_id

            logger.info(f"✓ Order placed successfully:")
            logger.info(f"  Status: {response.status}")
            logger.info(f"  Returned order ID: {self.returned_order_id}")

            # Step 5: Compare order IDs
            logger.info("\n[Step 5] Comparing order IDs...")

            if self.computed_order_id == self.returned_order_id:
                logger.info("✓ ORDER ID MATCH!")
                logger.info(f"  Computed: {self.computed_order_id}")
                logger.info(f"  Returned: {self.returned_order_id}")
                self.test_passed = True
            else:
                logger.error("✗ ORDER ID MISMATCH!")
                logger.error(f"  Computed: {self.computed_order_id}")
                logger.error(f"  Returned: {self.returned_order_id}")
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
            logger.info(f"  Computed order ID matches returned order ID")
            logger.info(f"  Order ID: {self.computed_order_id}")
        else:
            logger.error("✗ TEST FAILED")
            if self.computed_order_id and self.returned_order_id:
                logger.error(f"  Computed: {self.computed_order_id}")
                logger.error(f"  Returned: {self.returned_order_id}")

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