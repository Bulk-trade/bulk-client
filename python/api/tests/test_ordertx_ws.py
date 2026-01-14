"""
Test basic order operations on Bulk Labs exchange using WebSocket

Tests:
1. Limit order placement
2. Market order placement
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

from messages import LimitOrder

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from bulk.api import BulkHttpClient, BulkWebSocketClient
from bulk.common import Side, TimeInForce, OrderStatus
from bulk.common.signer import TransactionSigner

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


def get_current_price(http_client: BulkHttpClient, symbol: str) -> float:
    """Get current market price for a symbol using HTTP client"""
    try:
        ticker = http_client.get_ticker(symbol)
        price = ticker.get('lastPrice', 100000.0)
        logger.info(f"Current {symbol} price: ${price:,.2f}")
        return price
    except Exception as e:
        logger.error(f"Failed to get ticker: {e}")
        # Return a default price
        return 100000.0


async def test_limit_order(
    ws_client: BulkWebSocketClient,
    symbol: str,
    price: float
) -> tuple[bool, str]:
    """
    Test 1: Limit Order

    Places a limit order far from market and verifies response

    Returns:
        (success, order_id)
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 1: LIMIT ORDER (WebSocket)")
    logger.info("=" * 80)

    try:
        # Place limit order 5% below market (unlikely to fill)
        limit_price = price * 0.95

        logger.info(
            f"Placing limit order: BUY {0.001} @ ${limit_price:,.2f}"
        )

        # Use WebSocket client
        if True:
            response = await ws_client.place_limit_order(
                symbol=symbol,
                side=Side.BUY,
                price=limit_price,
                size=0.001,
                reduce_only=False,
                time_in_force=TimeInForce.GTC,
                timeout=10.0
            )
        else:
            response = await ws_client.place_orders([
                LimitOrder(
                    symbol=symbol,
                    side=Side.BUY,
                    price=limit_price,
                    size=0.001,
                    reduce_only=False,
                    time_in_force=TimeInForce.GTC)
            ])

        logger.info(f"Response: {response}")

        # Check response
        order_id = None
        if response.status == OrderStatus.RESTING:
            order_id = response.order_id
            logger.info(f"✓ Limit order placed successfully. Order ID: {order_id}")
            return True, order_id
        elif response.status == OrderStatus.FILLED:
            logger.info("✓ Limit order filled immediately")
            return True, None
        else:
            logger.error(f"✗ Order failed: {response.status} - {response.message}")
            return False, None

    except asyncio.TimeoutError:
        logger.error("✗ Limit order timed out")
        return False, None
    except Exception as e:
        logger.error(f"✗ Limit order failed: {e}")
        return False, None


async def test_market_order(
    ws_client: BulkWebSocketClient,
    symbol: str
) -> bool:
    """
    Test 2: Market Order

    Places a small market order and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 2: MARKET ORDER (WebSocket)")
    logger.info("=" * 80)

    try:
        logger.info(f"Placing market order: BUY {0.001} {symbol}")

        # Use WebSocket client
        response = await ws_client.place_market_order(
            symbol=symbol,
            side=Side.BUY,
            size=0.001,
            reduce_only=False,
            timeout=10.0
        )

        logger.info(f"Response: {response}")

        # Check response
        if response.status in [OrderStatus.FILLED, OrderStatus.PARTIALLY_FILLED]:
            logger.info("✓ Market order executed successfully")
            return True
        else:
            logger.error(f"✗ Market order failed: {response.status} - {response.message}")
            return False

    except asyncio.TimeoutError:
        logger.error("✗ Market order timed out")
        return False
    except Exception as e:
        logger.error(f"✗ Market order failed: {e}")
        return False


async def test_cancel_order(
    ws_client: BulkWebSocketClient,
    symbol: str,
    order_id: str
) -> bool:
    """
    Test 3: Cancel Order

    Cancels a specific order and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 3: CANCEL ORDER (WebSocket)")
    logger.info("=" * 80)

    if not order_id:
        logger.warning("⚠ No order ID provided, skipping cancel test")
        return True  # Not a failure, just skip

    try:
        logger.info(f"Cancelling order: {order_id}")

        # Use WebSocket client
        response = await ws_client.cancel_order(
            symbol=symbol,
            order_id=order_id,
            timeout=10.0
        )

        logger.info(f"Response: {response}")

        # Check response
        if response.status == OrderStatus.CANCELLED:
            logger.info("✓ Cancel order successful")
            return True
        else:
            logger.error(f"✗ Cancel failed: {response.status} - {response.message}")
            return False

    except asyncio.TimeoutError:
        logger.error("✗ Cancel order timed out")
        return False
    except Exception as e:
        logger.error(f"✗ Cancel order failed: {e}")
        return False


async def test_cancel_all(
    ws_client: BulkWebSocketClient,
    symbol: str,
    current_price: float
) -> bool:
    """
    Test 4: Cancel All Orders

    Cancels all orders for a symbol and verifies response
    """
    logger.info("\n" + "=" * 80)
    logger.info("TEST 4: CANCEL ALL ORDERS (WebSocket)")
    logger.info("=" * 80)

    try:
        # First, place a couple of orders to cancel
        logger.info("Placing 2 limit orders to test cancelAll...")

        await ws_client.place_orders([
            LimitOrder(
                symbol=symbol, side=Side.BUY, price=current_price * 0.95, size=0.001,
                time_in_force=TimeInForce.GTC),
            LimitOrder(
                symbol=symbol, side=Side.BUY, price=current_price * 0.94, size=0.001,
                time_in_force=TimeInForce.GTC),
        ])

        logger.info("✓ Placed test orders")

        # Wait a moment for orders to register
        await asyncio.sleep(1)

        # Now cancel all
        logger.info(f"Cancelling all orders for {symbol}")

        response = await ws_client.cancel_all(
            symbols=[symbol],
            timeout=10.0
        )

        logger.info(f"Response: {response}")

        # CancelAll returns OrderResponse - check if successful
        if response:
            logger.info("✓ Cancel all returned response (no timeout)")
            return True
        else:
            logger.error("✗ Cancel all returned empty response")
            return False

    except asyncio.TimeoutError:
        logger.error("✗ Cancel all timed out")
        return False
    except Exception as e:
        logger.error(f"✗ Cancel all failed: {e}")
        return False


async def run_tests():
    """Main test runner (async)"""
    logger.info("=" * 80)
    logger.info("BULK LABS ORDER OPERATIONS TEST (WebSocket)")
    logger.info("=" * 80)

    # Configuration
    symbol = "BTC-USD"
    http_base_url = "https://exchange-api2.bulk.trade/api/v1"
    ws_url = "wss://exchange-wss.bulk.trade"

    # Step 1: Load or create keys
    logger.info("\nStep 1: Loading or creating keys...")
    private_key, public_key = await get_funded_account()

    # Step 2: Initialize HTTP client (for faucet and ticker)
    logger.info("\nStep 2: Initializing HTTP client...")
    http_client = BulkHttpClient(
        base_url=http_base_url,
        private_key=private_key
    )
    logger.info("✓ HTTP client initialized")

    # Step 4: Get current price
    logger.info("\nStep 4: Getting current market price...")
    current_price = get_current_price(http_client, symbol)

    # Step 5: Initialize WebSocket client
    logger.info("\nStep 5: Connecting to WebSocket...")
    signer = TransactionSigner(private_key)
    ws_client = BulkWebSocketClient(
        url=ws_url,
        symbols=[symbol],
        signer=signer
    )

    # Connect
    connected = await ws_client.connect()
    if not connected:
        logger.error("Failed to connect to WebSocket")
        return 1

    logger.info("✓ WebSocket connected")

    # Wait for account snapshot
    logger.info("Waiting for account snapshot...")
    await asyncio.sleep(2)

    try:
        # Run tests
        results = {}

        # Test 1: Limit Order (and get order ID for cancel test)
        success, order_id = await test_limit_order(ws_client, symbol, current_price)
        results['limit_order'] = success
        await asyncio.sleep(1)

        # Test 4: Cancel All
        results['cancel_all'] = await test_cancel_all(ws_client, symbol, current_price)

        # Test 2: Market Order
        results['market_order'] = await test_market_order(ws_client, symbol)
        await asyncio.sleep(1)

        # Test 3: Cancel Order
        results['cancel_order'] = await test_cancel_order(ws_client, symbol, order_id)
        await asyncio.sleep(1)

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

    finally:
        # Cleanup
        logger.info("\nDisconnecting from WebSocket...")
        await ws_client.disconnect()
        logger.info("✓ Disconnected")


def main():
    """Entry point"""
    exit_code = asyncio.run(run_tests())
    sys.exit(exit_code)


if __name__ == "__main__":
    main()