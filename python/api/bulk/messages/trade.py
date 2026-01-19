import hashlib
import struct
import time
from dataclasses import dataclass
from typing import Dict, Optional, List

import base58
from bulk.common import OrderStatus, TimeInForce, Side
from bulk.common.signer import TransactionSigner
from bulk.messages import OrderState
from numba.parfors.parfor_lowering import redtyp_to_redarraytype

# =======================================================
# Serialization related
# =======================================================

TIME_IN_FORCE_MAP = {
    TimeInForce.GTC: 0,
    TimeInForce.IOC: 1,
    TimeInForce.ALO: 2,
}

SIDE_MAP = {
    Side.BUY: 0,
    Side.SELL: 1,
}

# 8 decimals
DECIMALS_MULTIPLIER = 100000000.0

def _write_u64(value: int) -> bytes:
    """Write a u64 in little-endian format"""
    return struct.pack("<Q", value)

def _write_string(value: str) -> bytes:
    """Write a string in little-endian format"""
    s_bytes = value.encode('utf-8')
    return _write_u32(len(s_bytes)) + s_bytes

def _write_bool(value: bool) -> bytes:
    """Write a boolean as single byte"""
    return bytes([1 if value else 0])

def _write_f64(value: float) -> bytes:
    """Write a f64 (double) in little-endian format"""
    return struct.pack("<d", value)

def _write_u32(value: int) -> bytes:
    """Write a u32 in little-endian format"""
    return struct.pack("<I", value)

def _write_u8(value: int) -> bytes:
    """Write a u8 in little-endian format"""
    return struct.pack("B", value)

def _write_pubkey(key: str) -> bytes:
    """Decode a base58 public key"""
    key_bytes = base58.b58decode(key)

    if len(key_bytes) != 32:
        raise ValueError(f"Key must be 32 bytes, got {len(key_bytes)}")
    return key_bytes


# =======================================================
# Limit Order
# =======================================================


@dataclass
class LimitOrder:
    """Limit order"""
    symbol: str
    side: Side
    price: float
    size: float
    reduce_only: bool = False
    time_in_force: TimeInForce = TimeInForce.GTC

    nonce: Optional[int] = None
    oid: Optional[str] = None
    pubkey: Optional[str] = None

    def order_id(self) -> str:
        """
        Generate hash used as order ID
        """
        if self.oid:
            return self.oid
        if not self.nonce and not self.pubkey:
            raise ValueError(f"Neither pubkey nor nonce are set for order: {self}")

        ser = b''.join([
            _write_u64(self.nonce),
            _write_string(self.symbol),
            _write_pubkey(self.pubkey),
            _write_u8(SIDE_MAP[self.side]),
            _write_u64(round(self.size * DECIMALS_MULTIPLIER)),
            _write_u64(round(self.price * DECIMALS_MULTIPLIER)),
            _write_u32(TIME_IN_FORCE_MAP[self.time_in_force]),
            _write_bool(self.reduce_only),
        ])

        hash = hashlib.sha256(ser).digest()
        self.oid = base58.b58encode(hash).decode('utf-8')
        return self.oid

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
                },
                'oid': self.oid if self.oid else "unknown",
            }
        }
        return order

    def to_state(self, status: OrderStatus) -> OrderState:
        """Create state for this order"""
        return OrderState(
            timestamp=time.time_ns(),
            symbol=self.symbol,
            order_id=self.order_id(),
            side=self.side,
            price=self.price,
            status=status,
            vwap=0.0,
            size=self.size,
            size_done=0.0,
            size_orig=self.size,
            is_maker=True
        )

    def __str__(self) -> str:
        parts = [f"{self.side.name} {self.size:.17g} {self.symbol} @ {self.price:.17g}"]

        if self.reduce_only:
            parts.append("reduce_only")
        if self.time_in_force != TimeInForce.GTC:
            parts.append(f"tif={self.time_in_force.name}")
        if self.oid:
            parts.append(f"oid={self.oid}")
        if self.nonce is not None:
            parts.append(f"nonce={self.nonce}")

        return f"LimitOrder({', '.join(parts)})"



# =======================================================
# Market Order
# =======================================================

@dataclass
class MarketOrder:
    """Market Order"""
    symbol: str
    side: Side
    size: float
    reduce_only: bool = False

    nonce: Optional[int] = None
    pubkey: Optional[str] = None
    oid: Optional[str] = None

    def order_id(self) -> str:
        """
        Generate hash used as order ID
        """
        if self.oid:
            return self.oid
        if not self.nonce and not self.pubkey:
            raise ValueError(f"Neither pubkey nor nonce are set for order: {self}")

        ser = b''.join([
            _write_u64(self.nonce),
            _write_string(self.symbol),
            _write_pubkey(self.pubkey),
            _write_u8(SIDE_MAP[self.side]),
            _write_u64(round(self.size * DECIMALS_MULTIPLIER)),
            _write_bool(self.reduce_only),
        ])

        hash = hashlib.sha256(ser).digest()
        self.oid = base58.b58encode(hash).decode('utf-8')
        return self.oid


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

    def to_state(self, status: OrderStatus, price: float = 0.0) -> OrderState:
        """Create state for this order"""
        return OrderState(
            timestamp=time.time_ns(),
            symbol=self.symbol,
            order_id=self.order_id(),
            side=self.side,
            price=0.0,
            status=status,
            vwap=0.0,
            size=self.size,
            size_done=0.0,
            size_orig=self.size,
            is_maker=False
        )

    def __str__(self) -> str:
        parts = [f"{self.side.name} {self.size:.17g} {self.symbol}"]

        if self.reduce_only:
            parts.append("reduce_only")
        if self.oid:
            parts.append(f"oid={self.oid}")
        if self.nonce is not None:
            parts.append(f"nonce={self.nonce}")

        return f"MarketOrder({', '.join(parts)})"


# =======================================================
# Order related
# =======================================================

@dataclass
class CancelOrder:
    """Cancel order"""
    symbol: str
    oid: str
    nonce: Optional[int] = None

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cancel": {
                'c': self.symbol,
                'oid': self.oid
            }
        }


@dataclass
class CancelAll:
    """Cancel all orders for symbol or across symbols"""
    symbols: List[str]
    nonce: Optional[int] = None

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cancelAll": {
                'c': self.symbols,
            }
        }


# =======================================================
# Order Responses
# =======================================================

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


# =======================================================
# Tests
# =======================================================

def test_limitorder_hash1():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=98000.0,
        size=1.0,
        nonce=123456789,
        time_in_force=TimeInForce.ALO,
        reduce_only=True,
    )
    pubkey = "4wBqpZM9xaSheZzJSMawUKKwhdpChKbZ5eu5ky4Vigw"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "9ih2tCrhdiNqX1erp3bfmhRNeu7dWvte9f9K3Toy9xUP"

def test_limitorder_hash2():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.SELL,
        price=94865.0,
        size=0.9170000,
        nonce=123456789,
        time_in_force=TimeInForce.ALO,
        reduce_only=True,
    )
    pubkey = "4wBqpZM9xaSheZzJSMawUKKwhdpChKbZ5eu5ky4Vigw"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "HHrRQSycSTkQ3Aaw16UGwBxYVEy585gvvZF11pTYT3K7"

def test_limitorder_hash3():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.SELL,
        price=95323.5,
        size=5.649,
        nonce=1768652193232748,
        time_in_force=TimeInForce.GTC,
        reduce_only=False,
    )
    pubkey = "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "EkeYuCxbuLuYtrc6uEZUj2G9SibE1nD4Q9JALVhgckrj"

def test_limitorder_hash4a():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.SELL,
        price=95204.75,
        size=0.6704,
        nonce=1768654732092639,
        time_in_force=TimeInForce.GTC,
        reduce_only=False,
    )
    pubkey = "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "4WNdBm6EeWRupGGxyujv1pyJyj39XtRfiD2veL2FvQN9"

def test_limitorder_hash5():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.SELL,
        price=95247.0,
        size=0.886599995,
        nonce=1768669537637142,
        time_in_force=TimeInForce.GTC,
        reduce_only=False,
    )
    pubkey = "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "8b6tCZdtjGZqhM4naJ1WaHbDvDcPD6vH7Ug3CZTDgrhf"

def test_marketorder_hash():
    order = MarketOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        size=1.0,
        nonce=123456789,
        reduce_only=True,
    )
    pubkey = "4wBqpZM9xaSheZzJSMawUKKwhdpChKbZ5eu5ky4Vigw"
    hash = order.hash(pubkey, nonce=order.nonce)
    assert hash == "HDoXpY5bNBrwnCp5B4MJSLLaagTkJUEhTH3cNZYGLXmj"


if __name__ == "__main__":
    test_limitorder_hash1()
    test_limitorder_hash2()
    test_limitorder_hash3()
    test_limitorder_hash4a()
    test_limitorder_hash5()
    test_marketorder_hash()