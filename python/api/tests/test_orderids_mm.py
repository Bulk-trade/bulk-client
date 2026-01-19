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
from bulk.messages.trade import LimitOrder, OrderResponse, CancelOrder
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

    async def evaluate_set(self, json: dict, n = None):
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

            nonce = int(time.time_ns() / 1000)
            payload = json["request"]["payload"]["action"]["orders"]
            actions = []

            n = n or len(payload)
            for i in range(n):
                ojson = payload[i]
                if "order" in ojson:
                    oinfo = ojson["order"]
                    order = LimitOrder(
                        symbol=oinfo["c"],
                        side=Side.BUY if oinfo["b"] else Side.SELL,
                        price=oinfo["px"],
                        size=oinfo["sz"],
                        time_in_force=TimeInForce.GTC,
                        nonce=nonce
                    )
                    order.oid = order.hash(self.signer.public_key, nonce=nonce)
                    actions.append(order)
                elif "cancel" in ojson:
                    oinfo = ojson["cancel"]
                    order = CancelOrder(
                        symbol=oinfo["c"],
                        oid=oinfo["oid"],
                        nonce=nonce
                    )
                    actions.append(order)

            # Step 3: Place the orders
            logger.info("\n[Step 4] Placing orders on exchange...")

            responses = await self.client.place_orders(actions)

            logger.info(f"\n[Step 5] Verifying order ID matches, sent {len(actions)} vs received {len(responses)}")
            test_passed = True
            for i in range(len(responses)):
                oid = actions[i].oid
                oid_response = responses[i].order_id
                if oid != oid_response:
                    logger.error(f"Response ID {oid_response} does not match order oid {oid}, for: {actions[i]}")
                    test_passed = False

            return test_passed

        except Exception as e:
            logger.error(f"Test error: {e}", exc_info=True)
            return False

        finally:
            # Disconnect
            logger.info("\n[Cleanup] Disconnecting...")
            await self.client.disconnect()
            logger.info("✓ Disconnected")


async def main():
    """Main test entry point"""

    # Run test
    signer = await get_funded_account()
    test = OrderIDTest(signer)

    msg = {"method": "post", "request": {"type": "action", "payload": {"action": {"type": "order", "orders": [
        {"order": {"c": "BTC-USD", "b": True, "px": 95247.0, "sz": 144.6762, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "HnNh9SQXrmPoQevMZJjpFr9LBvkyFRKgq8TEJGZiy63q"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95247.0, "sz": 144.6762001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "Bpgv3Wt5tfk2bMpBeMHe28DwLEXz7odmi54R9y27Q3Jj"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95247.0, "sz": 144.67620019999998, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "4A4DwMCwVsPLSDCnbkLaszDyMZrpvffcMKFW14dYxP81"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95247.0, "sz": 144.6762003, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "D3RxY3wdNSUSUCeqb5uHKC9RyxmYM3rN8aTGR5CyeQKy"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95247.0, "sz": 144.6762004, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "Ebv2krX4K4wSDoLbUtC28oekL7t4FMjQpSX3H7vdYZKw"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95246.75, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "CM1u2jHMHSH134YC5muZBphQdtHo9mK4KonQfQCWTrt9"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95246.75, "sz": 0.2500001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "HHXkuVxTDixNYfnASuKhXUmoTCNCJErVJoMhmtNWBZDQ"}},
        {"order": {"c": "BTC-USD", "b": True, "px": 95246.75, "sz": 0.2500002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "GkiP25U5BAcyo4bkEFJNnbVThM622VTgpHrPat6qRzwh"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.0, "sz": 0.8865999999999999, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "GSR2XYx5r5jDrmm9F4sX4BnVbeDeGG8PfxP8ti7f3xGU"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.0, "sz": 0.8866000999999999, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "BevB7uwa9mdwhAGYy3rBDovu5mUjbpHRzJqGqceZbFgB"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.0, "sz": 0.8866002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "4hts6uSSfVYNxZUEo6qAiRuNMwd4xjC9xtBBjk5Z6Q9R"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.0, "sz": 0.8866002999999999, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "9kQQexA6aJgPs7fWCGe1DvxZP3NNm4BjpcybYcTqWSNW"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.0, "sz": 0.8866004, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "FpmcvLfgs2QaJ7VrtcZv4pdwvdthJkw2UoHQ1sDRfmEB"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.25, "sz": 0.3116, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "42xjNCodcoAn1Hu1T7mLP7nS8kwgo5oui5xwcheD998Q"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.25, "sz": 0.3116001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "6mjpntEJsxMTbqV1TwbFeXi9mjU1GqafzGRms2Fg7xTN"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.25, "sz": 0.3116002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "9ySUbkTNN5u45C4yuZ1n95ghMYE7SMJHJ5wbFNJ8p5bt"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.25, "sz": 0.3116003, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "H38ZuLh4yDBM8JJbZKwkEMP7vb7BayJcSRfedyNJg9ED"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95247.25, "sz": 0.3116004, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "8xLiAJ5NKYLVSEfhsMaQPVNJmd1Fsw5gCT7ZYut6xAyq"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95248.0, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "2arXoAJDiHMsXbZ3P4sFNNQThi4BdqdRqMStg6Md1qE2"}},
        {"order": {"c": "BTC-USD", "b": False, "px": 95248.0, "sz": 0.2500001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "GW9WrMq5NL8ejwNCJSyHnsNanYjHZZas7z1WLfZhGmx1"}}],
        "nonce": 1768669537637142},
        "account": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signer": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signature": "2td4DGC2yggYrwDF9zEZK2RNJqw1hpDLZFbYjchUh8vBSmnYuQzZAYbbNqRycATusmGUyfjnaC7PWtWKC7RKPKyz"}}, "id": 5}

    
    await test.evaluate_set(msg)

    #await test.evaluate_set({"method": "post", "request": {"type": "action", "payload": {"action": {"type": "order", "orders": [{"order": {"c": "BTC-USD", "b": True, "px": 95441.75, "sz": 4.7152, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "CccKz1UkMLpHK14rssWRA4BMuAwwgQKCYmkfVdynqb4R"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95441.75, "sz": 4.715200100000001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "BRd9iSQtj2RZ3aSKfrmk4agCsHb9V87m23mnj5H1aYTz"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95441.75, "sz": 4.7152002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "GhzWombc6CDL4vMTtDSaTixCGUgZCSBsY3TmpK6CogG8"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95441.75, "sz": 4.7152003, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "SYmCTa8k8YCTFY3LNyw5QyNDC6W8vv2911XL3mkzg9M"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95441.75, "sz": 4.7152004000000005, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "BAbck1RKAiNLCVcTrAZdnxAWHSNpAuDawWZKiMEgFPs7"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95441.0, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "9QKezUg7otFX6WN75TMKnQ8fXwK8X8cmP4eDKWTr7Hpw"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95440.75, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "CgBu9xPKJjqcwc2kG1r6pRGqjRv1jHSMtWNSinyqucYv"}}, {"order": {"c": "BTC-USD", "b": True, "px": 95440.75, "sz": 0.2500001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "EDbv1G2JMaLrJhs6H6c9XWNYANqGzzJ36uxBBymNYAd2"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95441.75, "sz": 0.48819999999999997, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "C4KVyEuWPXgFGgCkkoj6kiGNXrjnDQMrYN6LvUrSdhxW"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95441.75, "sz": 0.48820009999999997, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "8tMMDqr6wyCJBWjzggHgJRfu8toqFdRjZFknc1Xgs1d8"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95441.75, "sz": 0.4882002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "H3Wx41EBAMjzYhK6u9iWpuLuS5sRVfkT66YrviD8Ma5X"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95441.75, "sz": 0.4882003, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "71XGMZCSdG6Ca1buJTopgWW7wT41vY7EYGB3hRDet3Tg"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95441.75, "sz": 0.4882004, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "DMfnL5Vw4ahtBRntQZeCbjzoJCzBpfqedeh2jXVEXjxD"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.0, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "D9zbRpv3raMvdxtie6SufgYvQGMMXmyUUhb78aFZiE9V"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.0, "sz": 0.2500001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "2b6ooEd73J9fnf3wt6ixNNmgqBs5wEnrxrXfh9Nae6Qo"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.0, "sz": 0.2500002, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "EW1Gmt2UpGGRHBtJExbVuLk6LXykUG6WfNPo2pwjUjTX"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.0, "sz": 0.2500003, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "92o2QfDXYma2RGErYnja4jqGaEVqDVvi3nCWejAUv5NB"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.5, "sz": 0.25, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "EQ4WW7fP2oBJKHGsEE18tJj72cbYSHKmcK8VbweASzUU"}}, {"order": {"c": "BTC-USD", "b": False, "px": 95442.5, "sz": 0.2500001, "r": False, "t": {"limit": {"tif": "GTC"}}, "oid": "75mcLTCEkjzSmXJajTA29eUDBNBE6E1zi6tZLxhiUEgJ"}}], "nonce": 1768663405068250}, "account": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signer": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signature": "2yRpagouaerDmem2bwvrHi8rsDEg2vMvQBVNSh657gmp4PoLzsqtemCgrrW8bkekT5uddmXhkTjFsdQvQVLHmBwb"}}, "id": 5})
    #await test.evaluate_set({"method": "post", "request": {"type": "action", "payload": {"action": {"type": "order", "orders": [{"order": {"c": "BTC-USD", "b": False, "sz": 3.97938946, "px": 0.0, "r": True, "t": {"trigger": {"is_market": True, "triggerPx": 0.0}}}}], "nonce": 399626944}, "account": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signer": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5", "signature": "4EuyQxbaJUQVvhq9f2mnm9xvX1LXGDCFr9WaA7zZ4WCaqNTzn6soruEzJdUqkQUhLzWJW6AQKNB36AH73UXZQSiY"}}, "id": 4})

    # Exit with appropriate code
    sys.exit(0)


if __name__ == "__main__":
    asyncio.run(main())