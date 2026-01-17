"""
Test simulated API
"""

import asyncio
import logging
import time

from api.bulk_simulated import Topic
from bulk.common import Side, TimeInForce, OrderStatus
from bulk.common.signer import TransactionSigner
from bulk.messages.md import Ticker, Trade, L2Snapshot, L2Delta, Candle, OrderBookLevel
from bulk.messages.trade import OrderResponse, Fill, CancelOrder, LimitOrder, CancelAll, MarketOrder
from bulk.data import FastOrderBook
from bulk.api import SimulatedWebSocketClient


async def test_simulated_client():
    """
    Comprehensive test suite for SimulatedWebSocketClient
    """
    import logging

    # Setup logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    logger.info("=" * 80)
    logger.info("SIMULATED WEBSOCKET CLIENT TEST SUITE")
    logger.info("=" * 80)

    # Create a test signer
    signer = TransactionSigner.generate_account()

    # Create order book
    bid_levels = [
        OrderBookLevel(price=100000.0 - i * 10.0, size=float(i + 1) * 0.5)
        for i in range(20)
    ]
    ask_levels = [
        OrderBookLevel(price=100010.0 + i * 10.0, size=float(i + 1) * 0.5)
        for i in range(20)
    ]

    snapshot = L2Snapshot(
        timestamp=int(time.time() * 1000),
        symbol="BTC-USD",
        bids=bid_levels,
        asks=ask_levels
    )

    orderbook = FastOrderBook("BTC-USD", max_levels=1000)
    orderbook.from_snapshot(snapshot)

    logger.info(f"\nOrderbook initialized:")
    logger.info(f"  Best Bid: ${orderbook.get_best_bid():,.2f}")
    logger.info(f"  Best Ask: ${orderbook.get_best_ask():,.2f}")
    logger.info(f"  Mid: ${orderbook.get_mid_price():,.2f}")
    logger.info(f"  Spread: ${orderbook.get_spread():.2f}")

    # Event tracking
    events = {
        Topic.ORDER: [],
        Topic.FILL: [],
        Topic.POSITION: [],
    }

    def track_event(event_type):
        def handler(data):
            events[event_type].append(data)
            if event_type == Topic.ORDER:
                logger.info(
                    f"  → ORDER EVENT: {data.status.name} | {data.order_id[:8]}... | {data.symbol} {data.side.name} {data.size}")
            elif event_type == Topic.FILL:
                logger.info(f"  → FILL EVENT: {data.symbol} {data.side.name} {data.size} @ ${data.price:,.2f}")
            elif event_type == Topic.POSITION:
                logger.info(f"  → POSITION UPDATE: {data.symbol} size={data.size:.4f} upnl=${data.unrealized_pnl:.2f}")

        return handler

    handlers = {
        Topic.ORDER: track_event(Topic.ORDER),
        Topic.FILL: track_event(Topic.FILL),
        Topic.POSITION: track_event(Topic.POSITION),
    }

    # Create client
    client = SimulatedWebSocketClient(
        symbols=["BTC-USD"],
        signer=signer,
        orderbook=orderbook,
        latency=0.01,  # 10ms for faster testing
        logger=logger,
        handlers=handlers,
    )

    # Connect
    await client.connect()
    logger.info("✓ Connected to simulated exchange\n")

    # ==================== TEST 1: Passive Limit Order (Should REST) ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 1: Passive Limit Order (Should REST)")
    logger.info("=" * 80)
    logger.info(f"Placing BUY limit @ $99,900 (below best bid of ${orderbook.get_best_bid():,.2f})")

    events[Topic.ORDER].clear()
    events[Topic.FILL].clear()

    response = await client.place_limit_order(
        symbol="BTC-USD",
        side=Side.BUY,
        price=99900.0,
        size=1.0
    )

    logger.info(f"Response: {response.status.name} | Order ID: {response.order_id[:16]}...")

    # Wait for events
    await asyncio.sleep(0.05)

    # Verify
    assert len(events[Topic.ORDER]) == 1, "Should have 1 order event"
    assert events[Topic.ORDER][0].status == OrderStatus.RESTING, "Order should be RESTING"
    assert len(events[Topic.FILL]) == 0, "Should have no fills"
    assert len(client.get_orders()) == 1, "Should have 1 open order"

    logger.info("✓ TEST 1 PASSED: Order is RESTING, no fill")

    # ==================== TEST 2: Aggressive Limit Order (Should FILL) ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 2: Aggressive Limit Order (Should FILL)")
    logger.info("=" * 80)
    logger.info(f"Placing BUY limit @ $100,020 (above best ask of ${orderbook.get_best_ask():,.2f})")

    events[Topic.ORDER].clear()
    events[Topic.FILL].clear()
    events[Topic.POSITION].clear()

    response = await client.place_limit_order(
        symbol="BTC-USD",
        side=Side.BUY,
        price=100020.0,
        size=0.5
    )

    logger.info(f"Response: {response.status.name} | Order ID: {response.order_id[:16]}...")

    # Wait for events
    await asyncio.sleep(0.05)

    # Verify
    assert len(events[Topic.ORDER]) >= 2, "Should have RESTING + FILLED events"
    assert events[Topic.ORDER][-1].status == OrderStatus.FILLED, "Final status should be FILLED"
    assert len(events[Topic.FILL]) == 1, "Should have 1 fill"
    assert events[Topic.FILL][0].size == 0.5, "Fill size should match"
    assert len(events[Topic.POSITION]) == 1, "Should have position update"
    assert len(client.get_orders()) == 1, "Should still have 1 open order (from TEST 1)"

    logger.info("✓ TEST 2 PASSED: Order FILLED immediately")

    # ==================== TEST 3: Market Order (Should FILL) ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 3: Market Order (Should FILL)")
    logger.info("=" * 80)
    logger.info(f"Placing SELL market order for 0.3 BTC")

    events[Topic.ORDER].clear()
    events[Topic.FILL].clear()
    events[Topic.POSITION].clear()

    response = await client.place_market_order(
        symbol="BTC-USD",
        side=Side.SELL,
        size=0.3
    )

    logger.info(f"Response: {response.status.name} | Order ID: {response.order_id[:16]}...")

    # Wait for events
    await asyncio.sleep(0.05)

    # Verify
    assert len(events[Topic.ORDER]) >= 2, "Should have RESTING + FILLED events"
    assert events[Topic.ORDER][-1].status == OrderStatus.FILLED, "Market order should FILL"
    assert len(events[Topic.FILL]) == 1, "Should have 1 fill"
    assert events[Topic.FILL][0].size == 0.3, "Fill size should match"
    assert events[Topic.FILL][0].side == Side.SELL, "Should be a SELL"

    # Check position
    pos = client.inventory.position_for("BTC-USD")
    expected_position = 0.5 - 0.3  # bought 0.5, sold 0.3
    assert abs(pos.quantity - expected_position) < 1e-6, f"Position should be {expected_position}, got {pos.quantity}"

    logger.info(f"✓ TEST 3 PASSED: Market order FILLED, position is {pos.quantity:.4f}")

    # ==================== TEST 4: Cancel Order ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 4: Cancel Order")
    logger.info("=" * 80)

    # First place an order to cancel
    response = await client.place_limit_order(
        symbol="BTC-USD",
        side=Side.SELL,
        price=100200.0,
        size=1.0
    )
    order_id = response.order_id
    logger.info(f"Placed order to cancel: {order_id[:16]}...")

    await asyncio.sleep(0.05)
    assert len(client.get_orders()) == 2, "Should have 2 open orders"

    # Now cancel it
    events[Topic.ORDER].clear()

    cancel_response = await client.cancel_order("BTC-USD", order_id)
    logger.info(f"Cancel response: {cancel_response.status.name}")

    await asyncio.sleep(0.02)

    # Verify
    assert cancel_response.status == OrderStatus.CANCELLED, "Cancel should succeed"
    assert len(events[Topic.ORDER]) == 1, "Should have 1 cancel event"
    assert events[Topic.ORDER][0].status == OrderStatus.CANCELLED, "Event should be CANCELLED"
    assert len(client.get_orders()) == 1, "Should have 1 open order remaining"

    logger.info("✓ TEST 4 PASSED: Order cancelled successfully")

    # ==================== TEST 5: Batch Order Placement ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 5: Batch Order Placement (Atomic)")
    logger.info("=" * 80)
    logger.info("Placing 3 orders in a single batch:")
    logger.info("  1. BUY limit @ $99,950 (passive)")
    logger.info("  2. SELL limit @ $100,100 (passive)")
    logger.info("  3. BUY market (aggressive)")

    events[Topic.ORDER].clear()
    events[Topic.FILL].clear()

    actions = [
        LimitOrder("BTC-USD", Side.BUY, 99950.0, 0.2),
        LimitOrder("BTC-USD", Side.SELL, 100100.0, 0.2),
        MarketOrder("BTC-USD", Side.BUY, 0.1),
    ]

    start_time = time.time()
    responses = await client.place_orders(actions)
    response_time = time.time() - start_time

    logger.info(f"Received {len(responses)} responses in {response_time * 1000:.1f}ms")
    for i, resp in enumerate(responses, 1):
        logger.info(f"  {i}. {resp.status.name} | {resp.order_id[:16]}...")

    # Wait for all events
    await asyncio.sleep(0.1)

    # Verify
    assert len(responses) == 3, "Should have 3 responses"
    assert all(r.status == OrderStatus.RESTING for r in responses), "All responses should be RESTING initially"

    # Should have events for all orders
    order_events = events[Topic.ORDER]
    fill_events = events[Topic.FILL]

    # Count final states
    resting_orders = [e for e in order_events if e.status == OrderStatus.RESTING]
    filled_orders = [e for e in order_events if e.status == OrderStatus.FILLED]

    assert len(filled_orders) >= 1, "Market order should have filled"
    assert len(fill_events) >= 1, "Should have at least 1 fill"

    logger.info(f"✓ TEST 5 PASSED: Batch placement completed")
    logger.info(f"  - {len(resting_orders)} orders RESTING")
    logger.info(f"  - {len(filled_orders)} orders FILLED")
    logger.info(f"  - {len(fill_events)} fills executed")

    # ==================== TEST 6: Cancel All ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 6: Cancel All Orders")
    logger.info("=" * 80)

    open_before = len(client.get_orders())
    logger.info(f"Open orders before cancel_all: {open_before}")

    events[Topic.ORDER].clear()

    num_cancelled = await client.cancel_all(symbols=["BTC-USD"])
    logger.info(f"Cancelled {num_cancelled} orders")

    await asyncio.sleep(0.05)

    # Verify
    assert num_cancelled == open_before, f"Should cancel all {open_before} orders"
    assert len(client.get_orders()) == 0, "Should have no open orders"

    cancelled_events = [e for e in events[Topic.ORDER] if e.status == OrderStatus.CANCELLED]
    assert len(cancelled_events) == open_before, "Should have cancel event for each order"

    logger.info("✓ TEST 6 PASSED: All orders cancelled")

    # ==================== TEST 7: Order Without Orderbook ====================
    logger.info("\n" + "=" * 80)
    logger.info("TEST 7: Order Without Orderbook (Market fills only)")
    logger.info("=" * 80)

    # Create client without orderbook
    client_no_book = SimulatedWebSocketClient(
        symbols=["ETH-USD"],
        signer=signer,
        orderbook=None,  # No orderbook
        latency=0.01,
        logger=logger,
    )

    await client_no_book.connect()
    client_no_book.update_price("ETH-USD", 4000.0)

    # Limit order should rest
    events_nb = {Topic.ORDER: [], Topic.FILL: []}
    client_no_book.on(Topic.ORDER, lambda e: events_nb[Topic.ORDER].append(e))
    client_no_book.on(Topic.FILL, lambda e: events_nb[Topic.FILL].append(e))

    resp_limit = await client_no_book.place_limit_order(
        "ETH-USD", Side.BUY, 3950.0, 1.0
    )
    await asyncio.sleep(0.05)

    assert len(events_nb[Topic.FILL]) == 0, "Limit order without book should not fill"
    assert len(client_no_book.get_orders()) == 1, "Order should be open"

    # Market order should fill at mid price
    resp_market = await client_no_book.place_market_order(
        "ETH-USD", Side.BUY, 0.5
    )
    await asyncio.sleep(0.05)

    assert len(events_nb[Topic.FILL]) == 1, "Market order should fill"
    assert abs(events_nb[Topic.FILL][0].price - 4000.0) < 1.0, "Should fill near mid price"

    logger.info("✓ TEST 7 PASSED: Market order fills without orderbook")

    await client_no_book.disconnect()

    # ==================== FINAL SUMMARY ====================
    logger.info("\n" + "=" * 80)
    logger.info("FINAL STATE")
    logger.info("=" * 80)

    # Check final inventory
    pos = client.inventory.position_for("BTC-USD")
    pnl = client.get_pnl()

    logger.info(f"\nPosition Summary:")
    logger.info(f"  BTC-USD: {pos.quantity:.4f} BTC")
    logger.info(f"  Entry VWAP: ${pos.vwap:,.2f}")
    logger.info(f"  Fair Price: ${pos.fair_price:,.2f}")
    logger.info(f"  Realized PnL: ${pos.realized_pnl:.2f}")
    logger.info(f"  Unrealized PnL: ${pos.unrealized_pnl:.2f}")

    logger.info(f"\nTotal P&L: ${pnl.net:.2f}")
    logger.info(f"  Realized: ${pnl.realized:.2f}")
    logger.info(f"  Unrealized: ${pnl.unrealized:.2f}")

    logger.info(f"\nOpen Orders: {len(client.get_orders())}")

    # Disconnect
    await client.disconnect()
    logger.info("\n✓ Disconnected from simulated exchange")

    logger.info("\n" + "=" * 80)
    logger.info("ALL TESTS PASSED ✓")
    logger.info("=" * 80)


if __name__ == "__main__":
    asyncio.run(test_simulated_client())