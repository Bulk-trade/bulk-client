import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from sortedcontainers import SortedDict

from bulk.common import Side

# ============================================================================
# Summary-level Data
# ============================================================================

@dataclass
class Ticker:
    """Market ticker data"""
    symbol: str
    last_price: float
    mark_price: float
    oracle_price: float
    price_change: float
    price_change_percent: float
    high_price: float
    low_price: float
    volume: float
    quote_volume: float
    open_interest: float
    funding_rate: float
    timestamp: int = field(default_factory=lambda: int(time.time() * 1000))

    @classmethod
    def from_api(cls, data: Dict) -> 'Ticker':
        return cls(
            symbol=data.get('symbol'),
            last_price=data.get('lastPrice'),
            mark_price=data.get('markPrice'),
            oracle_price=data.get('oraclePrice'),
            price_change=data.get('priceChange'),
            price_change_percent=data.get('priceChangePercent'),
            high_price=data.get('highPrice'),
            low_price=data.get('lowPrice'),
            volume=data.get('volume'),
            quote_volume=data.get('quoteVolume'),
            open_interest=data.get('openInterest'),
            funding_rate=data.get('fundingRate')
        )


@dataclass
class Candle:
    """OHLCV candlestick data"""
    symbol: str
    interval: str  # e.g., "1m", "5m", "1h"
    open_time: int  # t - milliseconds
    close_time: int  # T - milliseconds
    open: float
    high: float
    low: float
    close: float
    volume: float
    num_trades: int  # n

    @classmethod
    def from_api(cls, symbol: str, interval: str, data: Dict) -> 'Candle':
        return cls(
            symbol=symbol,
            interval=interval,
            open_time=data.get('t', 0),
            close_time=data.get('T', 0),
            open=data.get('o', 0.0),
            high=data.get('h', 0.0),
            low=data.get('l', 0.0),
            close=data.get('c', 0.0),
            volume=data.get('v', 0.0),
            num_trades=data.get('n', 0)
        )

    @classmethod
    def from_api_list(cls, symbol: str, interval: str, data: Dict) -> List['Candle']:
        """Parse list of candles from API response"""
        candles = data.get('candles', [])
        return [cls.from_api(symbol, interval, candle) for candle in candles]

# ============================================================================
# Trades
# ============================================================================

@dataclass
class Trade:
    """Trade structure"""
    timestamp: int
    symbol: str
    side: Side
    size: float
    price: float
    maker: str
    taker: str

    @classmethod
    def from_api(cls, data: Dict) -> List['Trade']:
        trades = []
        for trade in data["data"]:
            trades.append(cls(
                timestamp=trade.get('time'),
                symbol=trade.get('s'),
                size=trade.get('sz'),
                price=trade.get('px'),
                side=Side.BUY if trade.get('b') else Side.SELL,
                maker=trade.get('maker'),
                taker=trade.get('taker'),
            ))
        return trades


# ============================================================================
# Order Book Data Classes
# ============================================================================

@dataclass
class OrderBookLevel:
    """Single price level in the order book"""
    price: float  # px
    size: float  # sz
    num_orders: int = 0  # n - number of orders at this price level

    @classmethod
    def from_api(cls, data: Dict) -> 'OrderBookLevel':
        return cls(
            price=data.get('px', 0.0),
            size=data.get('sz', 0.0),
            num_orders=data.get('n', 0)
        )

    def __eq__(self, other):
        """Equality based on price for level matching"""
        if isinstance(other, OrderBookLevel):
            return self.price == other.price
        return False

    def __hash__(self):
        """Hash based on price for dict/set usage"""
        return hash(self.price)


@dataclass
class L2Snapshot:
    """Full order book snapshot"""
    symbol: str
    bids: List[OrderBookLevel]  # Sorted highest to lowest
    asks: List[OrderBookLevel]  # Sorted lowest to highest
    timestamp: int

    @classmethod
    def from_api(cls, data: Dict) -> 'L2Snapshot':
        """
        Parse L2 snapshot from API response.
        levels format: [[bid_levels], [ask_levels]]
        """
        book_data = data.get('book', {})
        levels = book_data.get('levels', [[], []])

        # Parse bids (index 0)
        bids = [OrderBookLevel.from_api(level) for level in levels[0]]
        # Parse asks (index 1)
        asks = [OrderBookLevel.from_api(level) for level in levels[1]]

        return cls(
            symbol=book_data.get('symbol', ''),
            bids=bids,
            asks=asks,
            timestamp=book_data.get('timestamp', 0))


@dataclass
class L2Delta:
    """Incremental order book update (delta)"""
    symbol: str
    bid_changes: List[OrderBookLevel]  # Bid level changes (empty if no bid changes)
    ask_changes: List[OrderBookLevel]  # Ask level changes (empty if no ask changes)
    timestamp: int

    @classmethod
    def from_api(cls, data: Dict) -> 'L2Delta':
        """
        Parse L2 delta from API response.
        levels format: [[bid_changes], [ask_changes]]
        Only one side will have changes per delta (the other will be empty)
        Size of 0 means delete the level
        """
        book_data = data.get('book', {})
        levels = book_data.get('levels', [[], []])

        # Parse bid changes (index 0)
        bid_changes = [OrderBookLevel.from_api(level) for level in levels[0]]
        # Parse ask changes (index 1)
        ask_changes = [OrderBookLevel.from_api(level) for level in levels[1]]

        return cls(
            symbol=book_data.get('symbol', ''),
            bid_changes=bid_changes,
            ask_changes=ask_changes,
            timestamp=book_data.get('timestamp', 0))


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


