"""
Comprehensive Market Order Test

This test demonstrates a complete trading workflow:
1. Generate or load Ed25519 keypair (saved to /tmp)
2. Request testnet USDC from faucet via HTTP API
3. Connect to WebSocket and subscribe to account updates
4. Place a market order
5. Monitor fills and order status
6. Display final account state

Usage:
    python test_market_order.py
"""

import asyncio
import json
import logging
import os
import time
from pathlib import Path
from typing import Tuple

from bulk.common import TransactionSigner, Side
from bulk.api import BulkHttpClient
from bulk.api import BulkWebSocketClient
from bulk.messages.trade import MarketOrder

# Configuration
KEYS_FILE = "/tmp/bulk_test_keys.json"
TESTNET_WS_URL = "wss://exchange-wss.bulk.trade"
TESTNET_HTTP_URL = "https://exchange-api2.bulk.trade/api/v1"

SYMBOL = "BTC-USD"
ORDER_SIZE = 0.01


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
    private_key, public_key = TransactionSigner.generate_account()

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


async def test_market_order():
    """Main test function for market order workflow"""

    # Configure logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    print("=" * 70)
    print("🚀 Bulk Labs Market Order Test")
    print("=" * 70)

    # ========== Step 1 + 2: Load keys + Funding ==========
    private_key, public_key = await get_funded_account()
    print(f"🔑 Public Key: {public_key}")

    # ========== Step 3: Initialize Clients ==========
    print("\n🔌 Connecting to Bulk Exchange WebSocket...")

    signer = TransactionSigner(private_key)
    ws_client = BulkWebSocketClient(
        url=TESTNET_WS_URL,
        signer=signer
    )

    # Track fills and order status
    fills_received = []
    order_status_updates = []

    # ========== Step 4: Register Event Handlers ==========

    def on_account_snapshot(snapshot):
        """Handle initial account snapshot"""
        print(f"\n📊 Account Snapshot Received:")
        print(f"   Balance: ${snapshot.margin.total_balance:,.2f}")
        print(f"   Available: ${snapshot.margin.available_balance:,.2f}")
        print(f"   Positions: {len(snapshot.positions)}")
        print(f"   Open Orders: {len(snapshot.open_orders)}")

        if snapshot.positions:
            print(f"\n   Current Positions:")
            for pos in snapshot.positions:
                print(f"     - {pos.symbol}: {pos.size:.4f} @ ${pos.entry_price:,.2f}")

    def on_fill(fill):
        """Handle trade fill"""
        fills_received.append(fill)
        side_emoji = "🟢" if fill.side == Side.BUY else "🔴"
        maker_badge = "M" if fill.is_maker else "T"

        print(f"\n{side_emoji} FILL RECEIVED [{maker_badge}]:")
        print(f"   Symbol: {fill.symbol}")
        print(f"   Side: {fill.side.name}")
        print(f"   Price: ${fill.price:,.2f}")
        print(f"   Size: {fill.size:.4f}")
        print(f"   Order ID: {fill.order_id}")
        print(f"   Timestamp: {fill.timestamp}")

    def on_order_placed(order):
        """Handle order placement confirmation"""
        order_status_updates.append(('placed', order))
        print(f"\n✅ ORDER PLACED:")
        print(f"   Order ID: {order.order_id}")
        print(f"   Symbol: {order.symbol}")
        print(f"   Side: {'BUY' if order.is_buy else 'SELL'}")
        print(f"   Price: ${order.price:,.2f}")
        print(f"   Size: {order.size:.4f}")

    def on_order_cancelled(order_id):
        """Handle order cancellation"""
        order_status_updates.append(('cancelled', order_id))
        print(f"\n❌ ORDER CANCELLED: {order_id}")

    def on_position_update(position):
        """Handle position update"""
        print(f"\n📍 POSITION UPDATE:")
        print(f"   Symbol: {position.symbol}")
        print(f"   Size: {position.size:.4f}")
        print(f"   Entry Price: ${position.entry_price:,.2f}")
        print(f"   Mark Price: ${position.mark_price:,.2f}")
        print(f"   Unrealized PnL: ${position.unrealized_pnl:,.2f}")

    def on_margin_update(margin):
        """Handle margin update"""
        print(f"\n💰 MARGIN UPDATE:")
        print(f"   Total Balance: ${margin.total_balance:,.2f}")
        print(f"   Available: ${margin.available_balance:,.2f}")
        print(f"   Used: ${margin.used_balance:,.2f}")

    # Register all handlers
    ws_client.on("account_snapshot", on_account_snapshot)
    ws_client.on("fill", on_fill)
    ws_client.on("order_placed", on_order_placed)
    ws_client.on("order_cancelled", on_order_cancelled)
    ws_client.on("position_update", on_position_update)
    ws_client.on("margin_update", on_margin_update)

    http_client = BulkHttpClient(base_url=TESTNET_HTTP_URL)
    try:
        # ========== Step 5: Connect and Subscribe ==========
        connected = await ws_client.connect()

        if not connected:
            logger.error("Failed to connect to WebSocket")
            return

        print("✅ Connected to WebSocket")

        # Subscribe to account updates
        print(f"\n📡 Subscribing to account updates for {public_key}...")
        await ws_client.subscribe_account(public_key)

        # Wait for account snapshot
        print("⏳ Waiting for account snapshot...")
        await asyncio.sleep(1)

        # ========== Step 6: Get Current Market Price ==========
        info = http_client.get_exchange_info()
        print(f"exchange info: {info}")

        if False:
            ticker = http_client.get_ticker(SYMBOL)
            current_price = ticker.get('lastPrice', 0)

            print(f"\n📈 Current {SYMBOL} Price: ${current_price:,.2f}")

        # ========== Step 7: Place Market Order ==========
        print(f"\n📤 Placing MARKET ORDER:")
        print(f"   Symbol: {SYMBOL}")
        print(f"   Side: BUY")
        print(f"   Size: {ORDER_SIZE}")

        # Create market order
        request_id = await ws_client.place_market_order(SYMBOL, Side.BUY, ORDER_SIZE)

        print(f"✅ Market order sent (Request ID: {request_id})")

        # ========== Step 8: Monitor for Fills ==========
        print("\n⏳ Monitoring for fills and order updates...")
        print("   (Waiting up to 10 seconds...)")

        start_time = time.time()
        timeout = 10  # seconds

        while time.time() - start_time < timeout:
            await asyncio.sleep(0.5)

            # Check if we received fills
            if fills_received:
                print(f"\n✅ Received {len(fills_received)} fill(s)")
                break

        # ========== Step 9: Summary ==========
        print("\n" + "=" * 70)
        print("📊 TEST SUMMARY")
        print("=" * 70)

        print(f"\n🔑 Account: {public_key}")

        if fills_received:
            print(f"\n✅ Fills Received: {len(fills_received)}")
            for i, fill in enumerate(fills_received, 1):
                print(f"\n   Fill #{i}:")
                print(f"   - Symbol: {fill.symbol}")
                print(f"   - Side: {fill.side.name}")
                print(f"   - Price: ${fill.price:,.2f}")
                print(f"   - Size: {fill.size:.4f}")
                print(f"   - Value: ${fill.price * fill.size:,.2f}")
                print(f"   - Maker: {fill.is_maker}")
        else:
            print(f"\n⚠️  No fills received (order may be resting or cancelled)")

        if order_status_updates:
            print(f"\n📋 Order Status Updates: {len(order_status_updates)}")
            for status, data in order_status_updates:
                if status == 'placed':
                    print(f"   - Order Placed: {data.order_id}")
                elif status == 'cancelled':
                    print(f"   - Order Cancelled: {data}")

        # Final account state
        if ws_client.margin:
            print(f"\n💰 Final Account State:")
            print(f"   Balance: ${ws_client.margin.total_balance:,.2f}")
            print(f"   Available: ${ws_client.margin.available_balance:,.2f}")

        if ws_client.inventory:
            print(f"\n📍 Open Positions: {ws_client.inventory.summary()}")

        if ws_client.open_orders:
            print(f"\n📋 Open Orders: {len(ws_client.open_orders)}")
            for order_id, order in ws_client.open_orders.items():
                print(f"   - {order.symbol}: {order.size:.4f} @ ${order.price:,.2f}")

        print("\n" + "=" * 70)
        print("✅ Test completed successfully!")
        print("=" * 70)

    except KeyboardInterrupt:
        print("\n\n⚠️  Test interrupted by user")

    except Exception as e:
        logger.error(f"Test error: {e}", exc_info=True)

    finally:
        # ========== Cleanup ==========
        print("\n🔌 Disconnecting from WebSocket...")
        await ws_client.disconnect()
        print("✅ Disconnected")


if __name__ == "__main__":
    print("\n" + "=" * 70)
    print("🧪 Bulk Labs Market Order Test Suite")
    print("=" * 70)
    print("\nThis test will:")
    print("  1. Generate or load Ed25519 keypair from /tmp")
    print("  2. Request testnet USDC from faucet")
    print("  3. Connect to WebSocket")
    print("  4. Place a market order")
    print("  5. Monitor fills and order status")
    print("\nPress Ctrl+C to stop at any time")
    print("=" * 70 + "\n")

    asyncio.run(test_market_order())