"""
Test basic order operations on Bulk Labs exchange

Tests:
1. Market order placement
2. Limit order placement
3. Cancel single order
4. Cancel all orders

Purpose: Verify each operation returns a response without timing out
"""

import asyncio
import json
import logging
import os
import sys
import time
from pathlib import Path
from typing import Tuple

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from bulk.api import BulkHttpClient
from bulk.common import Side, TimeInForce, OrderStatus
from bulk.common.signer import TransactionSigner
from bulk.messages import LimitOrder, MarketOrder, CancelOrder, CancelAll

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

KEYS_FILE = "/tmp/bulk_test_keys.json"
TESTNET_WS_URL = "wss://exchange-wss.bulk.trade"
TESTNET_HTTP_URL = "https://exchange-api2.bulk.trade/api/v1"


def load_or_generate_keys():
    """
    Load existing keys from /tmp or generate new ones

    Returns:
        tuple: (private_key_b58, public_key_b58, keypair_array)
    """
    keys_path = Path(KEYS_FILE)

    if keys_path.exists():
        print(f"📂 Loading existing keys from {KEYS_FILE}")
        with open(keys_path, 'r') as f:
            keys = json.load(f)
        return keys['private_key'], keys['public_key']

    print("🔑 Generating new Ed25519 keypair...")
    signer = TransactionSigner.generate_account()
    private_key, public_key = signer.private_key, signer.public_key

    # Save to file
    keys_data = {
        'private_key': private_key,
        'public_key': public_key,
    }

    with open(keys_path, 'w') as f:
        json.dump(keys_data, f, indent=2)

    print(f"💾 Saved keys to {KEYS_FILE}")
    return private_key, public_key

async def request_faucet_funds(private_key: str, public_key: str):
    """
    Request testnet USDC from faucet via HTTP API

    Args:
        private_key: Base58 encoded private key
        public_key: Base58 encoded public key
    """
    print("\n💰 Requesting testnet funds from faucet...")

    http_client = BulkHttpClient(
        base_url=TESTNET_HTTP_URL,
        private_key=private_key
    )

    try:
        result = http_client.request_faucet()
        print(f"✅ Faucet request successful: {result}")

        # Wait a moment for funds to be credited
        print("⏳ Waiting 3 seconds for funds to be credited...")
        await asyncio.sleep(3)

        # Check account balance
        account = http_client.get_full_account(public_key)
        if account and 'fullAccount' in account:
            margin = account['fullAccount'].get('marginSummary', {})
            balance = margin.get('totalBalance', 0)
            print(f"💵 Current balance: ${balance:,.2f} USDC")

        return True

    except Exception as e:
        print(f"❌ Faucet request failed: {e}")
        print("   (This may fail if you've already requested funds recently)")
        return False

async def get_funded_account() -> Tuple[str,str]:
    """
    Get funded account, creating and funding if not present
    """
    keys_path = Path(KEYS_FILE)
    if not keys_path.exists():
        private_key, public_key = load_or_generate_keys()
        await request_faucet_funds(private_key, public_key)
        return private_key, public_key
    else:
        return load_or_generate_keys()


def get_current_price(client: BulkHttpClient, symbol: str) -> float:
    """Get current market price for a symbol"""
    try:
        ticker = client.get_ticker(symbol)
        price = ticker.get('lastPrice', 100000.0)
        logger.info(f"Current {symbol} price: ${price:,.2f}")
        return price
    except Exception as e:
        logger.error(f"Failed to get ticker: {e}")
        # Return a default price
        return 100000.0


def test_market_order(client: BulkHttpClient, symbol: str) -> bool:
    """
    Test 1: Market Order

    Places a small market order and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 1: MARKET ORDER")
    logger.info("=" * 80)

    try:
        # Create small market order
        order = MarketOrder(
            symbol=symbol,
            side=Side.BUY,
            size=0.001,  # Very small size
            reduce_only=False
        )

        logger.info(f"Placing market order: {order.side.name} {order.size} {symbol}")

        result = client.place_orders([order])
        logger.info(f"Response: {json.dumps(result, indent=2)}")

        # Check if we got a response (not timeout)
        if result:
            logger.info("✓ Market order returned response (no timeout)")
            return True
        else:
            logger.error("✗ Market order returned empty response")
            return False

    except Exception as e:
        logger.error(f"✗ Market order failed: {e}")
        return False


def test_limit_order(client: BulkHttpClient, symbol: str, price: float) -> tuple[bool, str]:
    """
    Test 2: Limit Order

    Places a limit order far from market and verifies response

    Returns:
        (success, order_id)
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 2: LIMIT ORDER")
    logger.info("=" * 80)

    try:
        # Place limit order 5% below market (unlikely to fill)
        limit_price = price * 0.95

        order = LimitOrder(
            symbol=symbol,
            side=Side.BUY,
            price=limit_price,
            size=0.001,
            reduce_only=False,
            time_in_force=TimeInForce.GTC
        )

        logger.info(f"Placing limit order: {order.side.name} {order.size} @ ${order.price:,.2f}")

        result = client.place_orders([order])
        logger.info(f"Response: {json.dumps(result, indent=2)}")

        # Extract order ID if order was placed
        order_id = None
        if result and 'response' in result:
            response = result['response']
            if 'data' in response and 'statuses' in response['data']:
                statuses = response['data']['statuses']
                if statuses and len(statuses) > 0:
                    status = statuses[0]
                    if 'resting' in status:
                        order_id = status['resting'].get('oid')
                        logger.info(f"✓ Limit order placed successfully. Order ID: {order_id}")
                    elif 'filled' in status:
                        logger.info("✓ Limit order filled immediately")
                        return True, None
                    elif 'error' in status:
                        logger.error(f"✗ Order rejected: {status['error']}")
                        return False, None

        if order_id:
            return True, order_id
        else:
            logger.warning("⚠ Limit order returned response but no order ID found")
            return True, None  # Still count as success since we got a response

    except Exception as e:
        logger.error(f"✗ Limit order failed: {e}")
        return False, None


def test_cancel_order(client: BulkHttpClient, symbol: str, order_id: str) -> bool:
    """
    Test 3: Cancel Order

    Cancels a specific order and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 3: CANCEL ORDER")
    logger.info("=" * 80)

    if not order_id:
        logger.warning("⚠ No order ID provided, skipping cancel test")
        return True  # Not a failure, just skip

    try:
        cancel = CancelOrder(
            symbol=symbol,
            oid=order_id
        )

        logger.info(f"Cancelling order: {order_id}")

        result = client.place_orders([cancel])
        logger.info(f"Response: {json.dumps(result, indent=2)}")

        if result:
            logger.info("✓ Cancel order returned response (no timeout)")
            return True
        else:
            logger.error("✗ Cancel order returned empty response")
            return False

    except Exception as e:
        logger.error(f"✗ Cancel order failed: {e}")
        return False


def test_cancel_all(client: BulkHttpClient, symbol: str) -> bool:
    """
    Test 4: Cancel All Orders

    Cancels all orders for a symbol and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 4: CANCEL ALL ORDERS")
    logger.info("=" * 80)

    try:
        # First, place a couple of orders to cancel
        logger.info("Placing 2 limit orders to test cancelAll...")

        current_price = get_current_price(client, symbol)

        orders = [
            LimitOrder(
                symbol=symbol,
                side=Side.BUY,
                price=current_price * 0.95,
                size=0.001,
                time_in_force=TimeInForce.GTC
            ),
            LimitOrder(
                symbol=symbol,
                side=Side.BUY,
                price=current_price * 0.94,
                size=0.001,
                time_in_force=TimeInForce.GTC
            )
        ]

        client.place_orders(orders)
        logger.info("✓ Placed test orders")

        # Wait a moment for orders to register
        time.sleep(1)

        # Now cancel all
        cancel_all = CancelAll(
            symbols=[symbol]
        )

        logger.info(f"Cancelling all orders for {symbol}")

        result = client.place_orders([cancel_all])
        logger.info(f"Response: {json.dumps(result, indent=2)}")

        if result:
            logger.info("✓ Cancel all returned response (no timeout)")
            return True
        else:
            logger.error("✗ Cancel all returned empty response")
            return False

    except Exception as e:
        logger.error(f"✗ Cancel all failed: {e}")
        return False


async def main():
    """Main test runner"""
    logger.info("=" * 80)
    logger.info("BULK LABS ORDER OPERATIONS TEST")
    logger.info("=" * 80)

    # Configuration
    symbol = "BTC-USD"
    base_url = "https://exchange-api2.bulk.trade/api/v1"

    # Step 1: Load or create keys
    logger.info("\nStep 1: Loading or creating keys...")
    private_key, public_key = await get_funded_account()

    # Step 2: Initialize client
    logger.info("\nStep 2: Initializing client...")
    client = BulkHttpClient(
        base_url=base_url,
        private_key=private_key
    )
    logger.info("✓ Client initialized")

    # Wait a moment for funds to settle
    time.sleep(2)

    # Step 4: Get current price
    logger.info("\nStep 4: Getting current market price...")
    current_price = get_current_price(client, symbol)

    # Run tests
    results = {}

    # Test 1: Limit Order (and get order ID for cancel test)
    success, order_id = test_limit_order(client, symbol, current_price)
    results['limit_order'] = success
    time.sleep(1)

    # Test 2: Market Order
    results['market_order'] = test_market_order(client, symbol)
    time.sleep(1)

    # Test 3: Cancel Order
    results['cancel_order'] = test_cancel_order(client, symbol, order_id)
    time.sleep(1)

    # Test 4: Cancel All
    results['cancel_all'] = test_cancel_all(client, symbol)

    # Print summary
    logger.info("\n" + "=" * 80)
    logger.info("TEST SUMMARY")
    logger.info("=" * 80)

    all_passed = True
    for test_name, passed in results.items():
        status = "✓ PASSED" if passed else "✗ FAILED"
        logger.info(f"{test_name:20s}: {status}")
        if not passed:
            all_passed = False

    logger.info("=" * 80)

    if all_passed:
        logger.info("✓ ALL TESTS PASSED")
        return 0
    else:
        logger.info("✗ SOME TESTS FAILED")
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)