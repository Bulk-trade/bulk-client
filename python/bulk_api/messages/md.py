import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from sortedcontainers import SortedDict

from bulk_api.common import Side

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

    def __repr__(self) -> str:
        return f"OrderBookLevel(price={self.price}, size={self.size}, num_orders={self.num_orders})"

    def __str__(self) -> str:
        """Format as 'size @ price'"""
        return f"{self.size} @ {self.price}"

    def format_with_side(self, side: Side) -> str:
        """Format with Buy/Sell prefix"""
        side_str = "Buy" if side == Side.BUY else "Sell"
        return f"{side_str} {self.size} @ {self.price}"


@dataclass
class BBO:
    """Inside best bid offer"""
    timestamp: int
    symbol: str
    bid: OrderBookLevel = field(default_factory=OrderBookLevel)
    ask: OrderBookLevel = field(default_factory=OrderBookLevel)

    def spread(self):
        """Spread between ask and bid"""
        return self.ask.price - self.bid.price

    def __repr__(self) -> str:
        return f"BBO(bid=[{self.bid}], ask=[{self.ask}])"

    def __str__(self) -> str:
        """Format as 'size @ price'"""
        return f"BBO(bid=[{self.bid}], ask=[{self.ask}])"



@dataclass
class L2Snapshot:
    """Full order book snapshot"""
    timestamp: int
    symbol: str
    bids: List[OrderBookLevel]  # Sorted highest to lowest
    asks: List[OrderBookLevel]  # Sorted lowest to highest

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
            timestamp=book_data.get('timestamp', 0),
            symbol=book_data.get('symbol', ''),
            bids=bids,
            asks=asks)

    def sweep_px(self, side: Side, size: float) -> float:
        """
        Determine price to sweep given size
        """
        book = self.bids if side == Side.BUY else self.asks
        cumsize = 0.0
        for level in book:
            cumsize += level.size
            if cumsize >= size:
                return level.price

        return book[-1].price

    def liquidity_adj_mid(self, size: float) -> float:
        """
        Determine liquidity adjusted mid price
        """
        bid = self.sweep_px(Side.BUY, size)
        ask = self.sweep_px(Side.SELL, size)
        return (bid + ask) / 2.0


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
