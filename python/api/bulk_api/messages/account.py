import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

from bulk_api.common import Side, OrderStatus


@dataclass
class OrderState:
    """Represents an order status update"""
    timestamp: int
    symbol: str
    order_id: str
    status: OrderStatus
    side: Side
    price: float
    vwap: float
    size: float
    size_done: float
    size_orig: float
    is_maker: bool
    error: Optional[str] = None

    def get_side(self) -> Side:
        """Get the order side"""
        return Side.BUY if self.is_buy else Side.SELL

    def amount_remaining(self) -> float:
        """Get the amount of remaining to be filled"""
        return max(self.size - self.size_done, 0.0)

    @classmethod
    def from_api(cls, data: Dict) -> 'OrderState':
        return cls(
            timestamp=data.get('timestamp'),
            symbol=data.get('symbol'),
            order_id=data.get('orderId'),
            status=OrderStatus.from_string(data.get('status')),
            side=Side.BUY if data.get('isBuy') else Side.SELL,
            price=data.get('price'),
            vwap=data.get('vwap'),
            size=data.get('size'),
            size_done=data.get('filledSize', 0.0),
            size_orig=data.get('originalSize', data.get('size')),
            is_maker=data.get('maker', False),
            error = data.get('reason')
        )

@dataclass
class Margin:
    """Account-level margin information"""
    total_balance: float
    available_balance: float
    margin_used: float
    notional: float
    realized_pnl: float
    unrealized_pnl: float
    fees: float
    funding: float

    @classmethod
    def from_api(cls, data: Dict) -> 'Margin':
        return cls(
            total_balance=data.get('totalBalance', 0.0),
            available_balance=data.get('availableBalance', 0.0),
            margin_used=data.get('marginUsed', 0.0),
            notional=data.get('notional', 0.0),
            realized_pnl=data.get('realizedPnl', 0.0),
            unrealized_pnl=data.get('unrealizedPnl', 0.0),
            fees=data.get('fees', 0.0),
            funding=data.get('funding', 0.0)
        )


@dataclass
class Position:
    """Position information with full risk metrics"""
    symbol: str
    size: float  # Positive for long, negative for short
    price: float  # Volume-weighted average price (entry price)
    fair_price: float  # Current fair/mark price
    notional: float  # size × fair_price
    realized_pnl: float
    unrealized_pnl: float
    leverage: float
    liquidation_price: float
    fees: float
    funding: float
    maintenance_margin: float
    lambda_: float  # Lambda value (risk parameter)
    risk_allocation: float  # C_i

    @classmethod
    def from_api(cls, data: Dict) -> 'Position':
        return cls(
            symbol=data.get('symbol', ''),
            size=data.get('size', 0.0),
            price=data.get('price', 0.0),
            fair_price=data.get('fairPrice', 0.0),
            notional=data.get('notional', 0.0),
            realized_pnl=data.get('realizedPnl', 0.0),
            unrealized_pnl=data.get('unrealizedPnl', 0.0),
            leverage=data.get('leverage', 0.0),
            liquidation_price=data.get('liquidationPrice', 0.0),
            fees=data.get('fees', 0.0),
            funding=data.get('funding', 0.0),
            maintenance_margin=data.get('maintenanceMargin', 0.0),
            lambda_=data.get('lambda', 0.0),
            risk_allocation=data.get('riskAllocation', 0.0)
        )

    def is_long(self) -> bool:
        """Check if position is long"""
        return self.size > 0

    def is_short(self) -> bool:
        """Check if position is short"""
        return self.size < 0


@dataclass
class LeverageSetting:
    """Leverage setting for a symbol"""
    symbol: str
    leverage: float  # 1.0 to 50.0

    @classmethod
    def from_api(cls, data: Dict) -> 'LeverageSetting':
        return cls(
            symbol=data.get('symbol', ''),
            leverage=data.get('leverage', 1.0)
        )


@dataclass
class AccountSnapshot:
    """Complete account state snapshot"""
    margin: Margin
    positions: List[Position]
    open_orders: List[OrderState]
    leverage_settings: List[LeverageSetting]
    timestamp: int = field(default_factory=lambda: int(time.time() * 1000))

    @classmethod
    def from_api(cls, data: Dict) -> 'AccountSnapshot':
        return cls(
            margin=Margin.from_api(data.get('margin', {})),
            positions=[Position.from_api(pos) for pos in data.get('positions', [])],
            open_orders=[OrderState.from_api(order) for order in data.get('openOrders', [])],
            leverage_settings=[LeverageSetting.from_api(lev) for lev in data.get('leverageSettings', [])]
        )

    def get_position(self, symbol: str) -> Optional[Position]:
        """Get position for a specific symbol"""
        for pos in self.positions:
            if pos.symbol == symbol:
                return pos
        return None

    def get_leverage_setting(self, symbol: str) -> Optional[LeverageSetting]:
        """Get leverage setting for a specific symbol"""
        for lev in self.leverage_settings:
            if lev.symbol == symbol:
                return lev
        return None


@dataclass
class MarginUpdate:
    """Real-time margin update"""
    total_balance: float
    available_balance: float
    margin_used: float
    notional: float
    realized_pnl: float
    unrealized_pnl: float
    fees: float
    funding: float

    @classmethod
    def from_api(cls, data: Dict) -> 'MarginUpdate':
        return cls(
            total_balance=data.get('totalBalance', 0.0),
            available_balance=data.get('availableBalance', 0.0),
            margin_used=data.get('marginUsed', 0.0),
            notional=data.get('notional', 0.0),
            realized_pnl=data.get('realizedPnl', 0.0),
            unrealized_pnl=data.get('unrealizedPnl', 0.0),
            fees=data.get('fees', 0.0),
            funding=data.get('funding', 0.0)
        )


@dataclass
class PositionUpdate:
    """Real-time position update"""
    symbol: str
    size: float
    price: float
    realized_pnl: float
    unrealized_pnl: float
    leverage: float
    liquidation_price: float
    fair_price: float
    notional: float
    fees: float
    funding: float
    maintenance_margin: float
    lambda_: float
    risk_allocation: float

    @classmethod
    def from_api(cls, data: Dict) -> 'PositionUpdate':
        return cls(
            symbol=data.get('symbol', ''),
            size=data.get('size', 0.0),
            price=data.get('price', 0.0),
            realized_pnl=data.get('realizedPnl', 0.0),
            unrealized_pnl=data.get('unrealizedPnl', 0.0),
            leverage=data.get('leverage', 0.0),
            liquidation_price=data.get('liquidationPrice', 0.0),
            fair_price=data.get('fairPrice', 0.0),
            notional=data.get('notional', 0.0),
            fees=data.get('fees', 0.0),
            funding=data.get('funding', 0.0),
            maintenance_margin=data.get('maintenanceMargin', 0.0),
            lambda_=data.get('lambda', 0.0),
            risk_allocation=data.get('riskAllocation', 0.0)
        )
