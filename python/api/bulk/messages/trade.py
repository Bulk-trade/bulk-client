import time
from dataclasses import dataclass
from typing import Dict, Optional, List
from unittest import case

from bulk.common import OrderStatus, TimeInForce, Side
from bulk.common.signer import TransactionSigner

# ----------------------------------------------------------
# Order types
# ----------------------------------------------------------

@dataclass
class LimitOrder:
    """Limit order"""
    symbol: str
    side: Side
    price: float
    size: float
    nonce: int = time.time_ns()
    reduce_only: bool = False
    time_in_force: TimeInForce = TimeInForce.GTC

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            "order": {
                'c': self.symbol,
                'b': self.side.value == Side.BUY.value,
                'px': self.price,
                'sz': self.size,
                'r': self.reduce_only,
                't': {
                    'limit': {'tif': str(self.time_in_force)}
                }
            }
        }
        return order

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()],
                "nonce": self.nonce
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
    side: Side
    size: float
    nonce: int = time.time_ns()
    reduce_only: bool = False

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            "order": {
                'c': self.symbol,
                'b': self.side.value == Side.BUY.value,
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
        }
        return order

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()],
                "nonce": self.nonce
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
    nonce: int = time.time_ns()

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cancel": {
                'c': self.symbol,
                'oid': self.oid
            }
        }

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this cancel order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()],
                "nonce": self.nonce
            },
            "account": signer.public_key,
            "signer": signer.public_key,
        }
        tx = signer.sign_transaction(tx)
        return tx


@dataclass
class CancelAll:
    """Cancel all orders for symbol or across symbols"""
    symbols: List[str]
    nonce: int = time.time_ns()

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cancelAll": {
                'c': self.symbols,
            }
        }

    def to_tx(self, signer: TransactionSigner) -> Dict:
        """Create TX for this cancel order"""
        tx = {
            "action": {
                "type": "order",
                "orders": [self.to_api()],
                "nonce": self.nonce
            },
            "account": signer.public_key,
            "signer": signer.public_key,
        }
        tx = signer.sign_transaction(tx)
        return tx

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
class OrderResponse:
    """Represents an order post response"""
    order_id: Optional[str]
    status: OrderStatus
    message: Optional[str]
    meta: Dict

    def is_error(self) -> bool:
        """determine whether is an error"""
        match self.status:
            case OrderStatus.ERROR:
                return True
            case OrderStatus.REJECTED_RISKLIMIT:
                return True
            case OrderStatus.REJECTED_INVALID:
                return True
            case OrderStatus.REJECTED_DUPLICATE:
                return True
            case OrderStatus.REJECTED_CROSSING:
                return True
            case _:
                return False

    @classmethod
    def from_api(cls, data: Dict) -> List['OrderResponse']:
        rlist = data.get("data",{}).get("payload",{}).get("response",{}).get("data",{}).get("statuses", [])
        responses = []
        for response in rlist:
            match response:
                case {"error": body}:
                    # Handle error
                    responses.append(cls(order_id=None, status=OrderStatus.ERROR, message=body.get("message",None), meta=body))
                case _:
                    status_key = next(iter(response.keys()))
                    body = response[status_key]
                    status = OrderStatus.from_string(status_key)  # or OrderStatus[status_key.upper()]
                    responses.append(cls(
                        order_id=body.get("oid"),
                        status=status,
                        message=None,
                        meta=body
                    ))
        return responses

