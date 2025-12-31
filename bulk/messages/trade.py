from dataclasses import dataclass
from typing import Dict, Optional

from bulk.common import OrderStatus, TimeInForce, Side
from bulk.common.signer import TransactionSigner

# ----------------------------------------------------------
# Order types
# ----------------------------------------------------------

@dataclass
class LimitOrder:
    """Limit order"""
    symbol: str
    is_buy: bool
    price: float
    size: float
    reduce_only: bool = False
    time_in_force: TimeInForce = TimeInForce.GTC

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            #'type': 'limit',
            'c': self.symbol,
            'b': self.is_buy,
            'px': self.price,
            'sz': self.size,
            'r': self.reduce_only,
            't': {
                'limit': {'tif': self.time_in_force.value}
            }
        }
        return order

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()]
            },
            "account": signer.public_key,
            "signer": signer.public_key,
        }
        tx = signer.sign_transaction(tx)
        return tx

@dataclass
class MarketOrder:
    """Market Order"""
    symbol: str
    is_buy: bool
    size: float
    reduce_only: bool = False

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            #'type': 'market',
            'c': self.symbol,
            'b': self.is_buy,
            'sz': self.size,
            'px': 0.0,
            'r': self.reduce_only,
            't': {
                "trigger": {
                    "is_market": True,
                    "triggerPx": 0.0
                }
            }
        }
        return order

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()]
            },
            "account": signer.public_key,
            "signer": signer.public_key,
        }
        tx = signer.sign_transaction(tx)
        return tx

# ----------------------------------------------------------
# Order related
# ----------------------------------------------------------

@dataclass
class CancelOrder:
    """Cancel order"""
    symbol: str
    oid: str

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            #'type': 'cancel',
            'c': self.symbol,
            'oid': self.oid
        }

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this cancel order"""
        tx = {
            "action": {
                "type": "cancel",
                "cancels": [self.to_api()]
            },
            "account": signer.public_key,
            "signer": signer.public_key,
        }
        tx = signer.sign_transaction(tx)
        return tx


@dataclass
class CancelAll:
    """Cancel all orders for symbol or across symbols"""
    account: str
    symbol: Optional[str]

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        raise NotImplementedError("Cancel All Not Implemented")

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this cancel order"""
        raise NotImplementedError("Cancel All Not Implemented")


# ----------------------------------------------------------
# Order Responses
# ----------------------------------------------------------

@dataclass
class Fill:
    """Represents a trade fill"""
    symbol: str
    order_id: str
    client_id: Optional[str]
    price: float
    size: float
    side: Side
    timestamp: int
    is_maker: bool

    @classmethod
    def from_api(cls, data: Dict) -> 'Fill':
        return cls(
            symbol=data.get('symbol'),
            order_id=data.get('orderId'),
            client_id=data.get('clientId', None),
            price=data.get('price'),
            size=data.get('size'),
            side=Side.BUY if data.get('isBuy') else Side.SELL,
            timestamp=data.get('timestamp'),
            is_maker=data.get('maker', False)
        )

@dataclass
class OrderStatus:
    """Represents an order status update"""
    timestamp: int
    symbol: str
    order_id: str
    client_id: Optional[str]
    status: OrderStatus
    is_buy: bool
    price: float
    size: float
    size_done: float
    size_orig: float
    is_maker: bool

    @classmethod
    def from_api(cls, data: Dict) -> 'OrderStatus':
        return cls(
            timestamp=data.get('timestamp'),
            symbol=data.get('symbol'),
            order_id=data.get('orderId'),
            client_id=data.get('clientId', None),
            status=OrderStatus.from_string(data.get('status')),
            is_buy=data.get('isBuy'),
            price=data.get('price'),
            size=data.get('size'),
            size_done=data.get('size_done', 0.0),
            size_orig=data.get('size_orig', data.get('size')),
            is_maker=data.get('maker', False)
        )

