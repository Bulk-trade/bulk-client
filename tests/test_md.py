from bulk.api import BulkWebSocketClient

async def test_market_data():
    """Example: Subscribe to market data and print updates"""

    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # Create WebSocket client
    client = BulkWebSocketClient(
        url="wss://exchange-wss.bulk.trade"
    )

    # ========== Register Event Handlers ==========

    def on_ticker(ticker):
        """Handle ticker updates"""
        print(f"\n[TICKER] {ticker.symbol}")
        print(f"  Last Price: ${ticker.last_price:,.2f}")
        print(f"  24h Volume: {ticker.volume:,.2f}")
        print(f"  24h Change: {ticker.price_change_percent:+.2f}%")
        print(f"  High: ${ticker.high_price:,.2f} | Low: ${ticker.low_price:,.2f}")

    def on_trades(trades):
        """Handle trade updates"""
        for trade in trades:
            side_emoji = "🟢" if trade.side == Side.BUY else "🔴"
            print(f"[TRADE] {side_emoji} {trade.symbol}: "
                  f"{trade.side.name} {trade.size:.4f} @ ${trade.price:,.2f} "
                  f"[{trade.timestamp}]")

    def on_l2_snapshot(snapshot):
        """Handle order book snapshot"""
        print(f"\n[BOOK SNAPSHOT] {snapshot.symbol}")
        print(f"  Timestamp: {snapshot.timestamp}")
        print(f"  Bids: {len(snapshot.bids)} levels")
        print(f"  Asks: {len(snapshot.asks)} levels")

        # Print top 5 levels
        print("\n  Top 5 Bids:")
        for i, bid in enumerate(snapshot.bids[:5]):
            print(f"    {i + 1}. ${bid.price:,.2f} x {bid.size:.4f}")

        print("\n  Top 5 Asks:")
        for i, ask in enumerate(snapshot.asks[:5]):
            print(f"    {i + 1}. ${ask.price:,.2f} x {ask.size:.4f}")

    def on_l2_delta(delta):
        """Handle order book delta"""
        if delta.bid_changes or delta.ask_changes:
            print(f"[BOOK DELTA] {delta.symbol}: "
                  f"Bid Δ={len(delta.bid_changes)}, "
                  f"Ask Δ={len(delta.ask_changes)}")

    def on_candle(candle):
        """Handle candle updates"""
        print(f"\n[CANDLE] {candle.symbol} [{candle.interval}]")
        print(f"  Open:   ${candle.open:,.2f}")
        print(f"  High:   ${candle.high:,.2f}")
        print(f"  Low:    ${candle.low:,.2f}")
        print(f"  Close:  ${candle.close:,.2f}")
        print(f"  Volume: {candle.volume:,.2f}")

    # Register all handlers
    client.on("ticker", on_ticker)
    client.on("trades", on_trades)
    client.on("l2_snapshot", on_l2_snapshot)
    client.on("l2_delta", on_l2_delta)
    client.on("candle", on_candle)

    try:
        # ========== Connect ==========
        logger.info("Connecting to Bulk Exchange WebSocket...")
        connected = await client.connect()

        if not connected:
            logger.error("Failed to connect to WebSocket")
            return

        logger.info("✓ Connected successfully")

        # ========== Subscribe to Market Data ==========
        symbol = "BTC-USD"

        logger.info(f"\nSubscribing to market data for {symbol}...")

        # Subscribe to ticker (real-time price updates)
        await client.subscribe_ticker(symbol)
        logger.info(f"✓ Subscribed to ticker")

        # Subscribe to trades (real-time trade feed)
        await client.subscribe_trades([symbol])
        logger.info(f"✓ Subscribed to trades")

        # Subscribe to order book snapshot (initial state)
        #await client.subscribe_orderbook_snapshot(symbol, nlevels=1000)
        #logger.info(f"✓ Subscribed to order book snapshots")

        # Subscribe to order book deltas (real-time updates)
        await client.subscribe_orderbook_delta(symbol)
        logger.info(f"✓ Subscribed to order book deltas")

        # Subscribe to candles (OHLCV data)
        await client.subscribe_candles(symbol, "1m")
        logger.info(f"✓ Subscribed to 1m candles")

        logger.info(f"\n{'=' * 60}")
        logger.info("Receiving market data (press Ctrl+C to stop)...")
        logger.info(f"{'=' * 60}\n")

        # ========== Keep Running ==========
        # Print order book state every 30 seconds
        counter = 0
        while True:
            await asyncio.sleep(1)
            print("next second")
            counter += 1

            if counter % 30 == 0:
                # Get and print current order book state
                book = client.get_order_book(symbol)
                best_bid = book.get_best_bid()
                best_ask = book.get_best_ask()

                if best_bid and best_ask:
                    spread = best_ask.price - best_bid.price
                    spread_bps = (spread / best_bid.price) * 10000
                    mid_price = (best_bid.price + best_ask.price) / 2

                    print(f"\n{'─' * 60}")
                    print(f"[ORDER BOOK STATE] {symbol}")
                    print(f"  Mid Price: ${mid_price:,.2f}")
                    print(f"  Best Bid:  ${best_bid.price:,.2f} x {best_bid.size:.4f}")
                    print(f"  Best Ask:  ${best_ask.price:,.2f} x {best_ask.size:.4f}")
                    print(f"  Spread:    ${spread:.2f} ({spread_bps:.2f} bps)")
                    print(f"{'─' * 60}\n")

                # Get and print latest ticker
                ticker = client.get_ticker(symbol)
                print(f"[TICKER SUMMARY] {ticker.symbol}: "
                      f"Last=${ticker.last_price:,.2f}, "
                      f"Change={ticker.price_change_percent:+.2f}%, "
                      f"Volume={ticker.volume:,.0f}")

    except KeyboardInterrupt:
        logger.info("\n\nReceived shutdown signal...")

    except Exception as e:
        logger.error(f"Error in main loop: {e}", exc_info=True)

    finally:
        # ========== Cleanup ==========
        logger.info("Disconnecting from WebSocket...")
        await client.disconnect()
        logger.info("✓ Disconnected")

if __name__ == "__main__":
    import asyncio
    import logging

    # Run the async main function
    asyncio.run(test_market_data())