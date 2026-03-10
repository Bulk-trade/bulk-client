from bulk_api.api import BulkWebSocketClient
from bulk_api.api.ws import Topic
from bulk_api.common import Side
from bulk_api.messages import L2Snapshot, Ticker

tick = None
snap = None

def approx_funding(ticker: Ticker, snap: L2Snapshot, r = 0.0001):
    """Compute very approximate funding for a ticker."""
    pbid = snap.sweep_px(Side.BUY, 0.5)
    pask = snap.sweep_px(Side.SELL, 0.5)
    poracle = ticker.oracle_price

    delta = max(pbid - poracle, 0.0) - max(poracle - pask, 0.0)
    prate = delta / poracle
    funding = r + prate
    return funding


async def test_market_data():
    """Example: Subscribe to market data and print updates"""

    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    logger = logging.getLogger(__name__)

    # ========== Event Handlers ==========

    def on_ticker(ticker):
        """Handle ticker updates"""
        global tick
        if not tick:
            print(f"\n[TICKER] {ticker}")
            tick = ticker

    def on_l2_snapshot(snapshot):
        """Handle order book snapshot"""
        global snap
        if not snap:
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

            snap = snapshot

    # ========== Connect ==========
    logger.info("Connecting to Bulk Exchange WebSocket...")

    # Create WebSocket client
    client = BulkWebSocketClient(
        url="wss://exchange-wss.bulk.trade",
        symbols = ["BTC-USD"],
        handlers={
            Topic.TICKER: on_ticker,
            Topic.L2SNAPSHOT: on_l2_snapshot,
        }
    )
    connected = await client.connect()
    if not connected:
        logger.error("Failed to connect to WebSocket")
        return

    await client.subscribe_orderbook_snapshot("BTC-USD", 100)

    # ========== Keep Running Until Events Arrive ==========
    while not tick or not snap:
        await asyncio.sleep(1)

    funding = approx_funding(tick, snap)
    print(f"Funding: {funding}")

if __name__ == "__main__":
    import asyncio
    import logging

    # Run the async main function
    asyncio.run(test_market_data())