from dataclasses import dataclass
from typing import Optional, List, Dict

from sortedcontainers import SortedDict

from bulk_api.common import Side
from bulk_api.messages import L2Snapshot, L2Delta, BBO, OrderBookLevel


class OrderBook:
    """
    Stateful order book with efficient sorted access.
    Uses SortedDict for O(1) best bid/ask queries.
    """

    def __init__(self, symbol: str, decimals=9):
        self.symbol = symbol
        self.scale = pow(10, decimals)
        # SortedDict keeps keys in sorted order
        # For bids: use negative prices so highest bid comes first
        # For asks: use positive prices so lowest ask comes first
        self._bids: SortedDict[int, OrderBookLevel] = SortedDict()
        self._asks: SortedDict[int, OrderBookLevel] = SortedDict()
        self.last_update_time: int = 0

    def from_snapshot(self, snapshot: L2Snapshot) -> None:
        """Initialize order book from a full snapshot"""
        if snapshot.symbol != self.symbol:
            raise ValueError(f"Symbol mismatch: expected {self.symbol}, got {snapshot.symbol}")

        # Clear existing state
        self._bids.clear()
        self._asks.clear()

        # Add all bid levels (negative key for reverse sort)
        for level in snapshot.bids:
            iprice = int(round(level.price * self.scale))
            self._bids[-iprice] = level  # Negative for highest-first order

        # Add all ask levels (positive key for normal sort)
        for level in snapshot.asks:
            iprice = int(round(level.price * self.scale))
            self._asks[iprice] = level

        self.last_update_time = snapshot.timestamp

    def apply_delta(self, delta: L2Delta) -> None:
        """
        Apply incremental update to the order book.
        - If size > 0: Add or update the level
        - If size == 0: Delete the level
        """
        if delta.symbol != self.symbol:
            raise ValueError(f"Symbol mismatch: expected {self.symbol}, got {delta.symbol}")

        # Apply bid changes
        for level in delta.bid_changes:
            iprice = int(round(level.price * self.scale))
            if level.size == 0:
                self._bids.pop(-iprice, None)
            else:
                self._bids[-iprice] = level

        # Apply ask changes
        for level in delta.ask_changes:
            iprice = int(round(level.price * self.scale))
            if level.size == 0:
                self._asks.pop(iprice, None)
            else:
                self._asks[iprice] = level

        self.last_update_time = delta.timestamp


    def apply_bbo(self, bbo: BBO):
        """
        Apply incremental bid/ask update to the order book.
        """
        # Calculate integer prices
        bid_iprice = int(round(bbo.bid.price * self.scale))
        ask_iprice = int(round(bbo.ask.price * self.scale))

        # Remove bids with price > BBO bid (keys < -bid_iprice)
        # Since keys are negative, more negative = higher price
        while self._bids and self._bids.peekitem(0)[0] < -bid_iprice:
            self._bids.popitem(0)

        # Remove asks with price < BBO ask (keys < ask_iprice)
        # Since keys are positive, smaller key = lower price
        while self._asks and self._asks.peekitem(0)[0] < ask_iprice:
            self._asks.popitem(0)

        # Update BBO levels
        if bbo.bid.size > 0:
            self._bids[-bid_iprice] = bbo.bid
        if bbo.ask.size > 0:
            self._asks[ask_iprice] = bbo.ask

        self.last_update_time = bbo.timestamp


    def get_bids(self, n_levels: Optional[int] = None) -> List[OrderBookLevel]:
        """
        Get bid levels sorted from highest to lowest price.
        O(k) where k = n_levels (no sorting needed!)
        """
        if n_levels is None:
            return list(self._bids.values())
        return list(self._bids.values())[:n_levels]

    def get_asks(self, n_levels: Optional[int] = None) -> List[OrderBookLevel]:
        """
        Get ask levels sorted from lowest to highest price.
        O(k) where k = n_levels (no sorting needed!)
        """
        if n_levels is None:
            return list(self._asks.values())
        return list(self._asks.values())[:n_levels]

    def get_best_bid(self) -> Optional[OrderBookLevel]:
        """Get the best (highest) bid - O(1)"""
        if not self._bids:
            return None
        # First key is most negative = highest price
        return self._bids.peekitem(0)[1]

    def get_best_ask(self) -> Optional[OrderBookLevel]:
        """Get the best (lowest) ask - O(1)"""
        if not self._asks:
            return None
        # First key is lowest price
        return self._asks.peekitem(0)[1]

    def get_spread(self) -> Optional[float]:
        """Get the bid-ask spread - O(1)"""
        best_bid = self.get_best_bid()
        best_ask = self.get_best_ask()
        if best_bid and best_ask:
            return best_ask.price - best_bid.price
        return None

    def get_mid_price(self) -> Optional[float]:
        """Get the mid price - O(1)"""
        best_bid = self.get_best_bid()
        best_ask = self.get_best_ask()
        if best_bid and best_ask:
            return (best_bid.price + best_ask.price) / 2
        return None

    def get_depth(self, side: str, depth_price: float) -> float:
        """
        Calculate total size from best price to depth_price.
        O(k) where k is number of levels in range.
        """
        total_size = 0.0
        iprice = int(round(depth_price * self.scale))

        if side.lower() == 'bid':
            # Iterate from best bid down to depth_price
            # Keys are negative, so we want keys >= -iprice
            for key, level in self._bids.items():
                if key >= -iprice:  # level.price >= depth_price
                    total_size += level.size
                else:
                    break  # Sorted, so we can stop early

        elif side.lower() == 'ask':
            # Iterate from best ask up to depth_price
            # Keys are positive, so we want keys <= iprice
            for key, level in self._asks.items():
                if key <= iprice:  # level.price <= depth_price
                    total_size += level.size
                else:
                    break  # Sorted, so we can stop early

        return total_size

    def get_level_at_price(self, price: float, side: Side) -> Optional[OrderBookLevel]:
        """
        Get the order book level at a specific price - O(log n)
        """
        iprice = int(round(price * self.scale))
        if side == Side.BUY:
            return self._bids.get(-iprice)
        elif side == Side.SELL:
            return self._asks.get(iprice)
        return None

    def __repr__(self) -> str:
        return f"OrderBook(symbol={self.symbol}, bids={len(self.bids)}, asks={len(self.asks)})"

    def __str__(self) -> str:
        """Format order book with sells first, then buys"""
        return self.format_book(depth=10)

    def format_book(self, depth: int = 10) -> str:
        """
        Format order book for display
        """
        lines = [f"{self.symbol} Order Book:"]

        # Get sorted levels
        asks = sorted(self.get_asks(), key=lambda x: x.price, reverse=True)[:depth]
        bids = sorted(self.get_bids(), key=lambda x: x.price, reverse=True)[:depth]

        # Sells (asks) - highest to lowest (furthest from mid to closest)
        if asks:
            lines.append("\nSells:")
            for ask in asks:
                lines.append(f"  Sell {ask.size:>10.4f} @ {ask.price:.4f}")

        # Spread indicator
        best_bid = self.get_best_bid()
        best_ask = self.get_best_ask()
        if best_bid and best_ask:
            spread = best_ask.price - best_bid.price
            mid = (best_bid.price + best_ask.price) / 2
            lines.append(f"\n  {'---':>10} Spread: {spread:.4f} Mid: {mid:.4f}")

        # Buys (bids) - highest to lowest (closest to mid to furthest)
        if bids:
            lines.append("\nBuys:")
            for bid in bids:
                lines.append(f"  Buy  {bid.size:>10.4f} @ {bid.price:.4f}")

        return "\n".join(lines)
