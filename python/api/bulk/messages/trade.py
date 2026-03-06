import hashlib
import json
import struct
import time
from dataclasses import dataclass
from typing import Dict, Optional, List

import base58
from bulk.common import OrderStatus, TimeInForce, Side
from bulk.messages import OrderState


# =======================================================
# Serialization related
# =======================================================

TIME_IN_FORCE_MAP = {
    TimeInForce.GTC: 0,
    TimeInForce.IOC: 1,
    TimeInForce.ALO: 2,
}

SIDE_MAP = {
    Side.BUY: 1,
    Side.SELL: 0,
}

# 8 decimals
DECIMALS_MULTIPLIER = 100000000.0

def _write_u64(value: int) -> bytes:
    """Write a u64 in little-endian format"""
    return struct.pack("<Q", value)

def _write_string(value: str) -> bytes:
    """Write a string in little-endian format"""
    s_bytes = value.encode('utf-8')
    return _write_u64(len(s_bytes)) + s_bytes

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
# Oracle Prices
# =======================================================

@dataclass
class OraclePrice:
    """oracle price container"""
    timestamp: int
    symbol: str
    price: float
    nonce: Optional[str] = None
    pubkey: Optional[str] = None

    def order_id(self) -> Optional[str]:
        return None

    def to_api(self) -> List[Dict]:
        """Convert to API format with compact field names"""
        return {
            "px": {
                't': self.timestamp,
                'c': self.symbol,
                'px': float(self.price)
            }
        }

    def __str__(self) -> str:
        return json.dumps(self.to_api())



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

    nonce: Optional[str] = None
    oid: Optional[str] = None
    pubkey: Optional[str] = None

    def order_id(self) -> Optional[str]:
        """
        Generate hash used as order ID
        """
        if self.oid:
            return self.oid
        if not self.nonce and not self.pubkey:
            raise ValueError(f"Neither pubkey nor nonce are set for order: {self}")

        ser = b''.join([
            _write_u32(1),
            _write_string(self.symbol),
            _write_u8(SIDE_MAP[self.side]),
            _write_u64(round(self.price * DECIMALS_MULTIPLIER)),
            _write_u64(round(self.size * DECIMALS_MULTIPLIER)),
            _write_u32(TIME_IN_FORCE_MAP[self.time_in_force]),
            _write_bool(self.reduce_only),
            _write_pubkey(self.pubkey),
            _write_u64(int(self.nonce)),
        ])

        dec = list(ser)

        hash = hashlib.sha256(ser).digest()
        self.oid = base58.b58encode(hash).decode('utf-8')
        return self.oid

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            "l": {
                'c': self.symbol,
                'b': self.side.value == Side.BUY.value,
                'px': f"{self.price:.8g}",
                'sz': f"{self.size:.8g}",
                'r': self.reduce_only,
                'tif': str(self.time_in_force)
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
        parts = [f"{self.side.name} {self.size:.17g} {self.symbol} @ {self.price:.17g}, tif={self.time_in_force.name}"]

        if self.reduce_only:
            parts.append("reduce_only")
        if self.oid:
            parts.append(f"oid={self.oid}")
        if self.pubkey:
            parts.append(f"account={self.pubkey}")
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

    nonce: Optional[str] = None
    pubkey: Optional[str] = None
    oid: Optional[str] = None

    def order_id(self) -> Optional[str]:
        """
        Generate hash used as order ID
        """
        if self.oid:
            return self.oid
        if not self.nonce and not self.pubkey:
            raise ValueError(f"Neither pubkey nor nonce are set for order: {self}")

        ser = b''.join([
            _write_u32(0),
            _write_string(self.symbol),
            _write_u8(SIDE_MAP[self.side]),
            _write_u64(round(self.size * DECIMALS_MULTIPLIER)),
            _write_bool(self.reduce_only),
            _write_pubkey(self.pubkey),
            _write_u64(int(self.nonce)),
        ])

        hash = hashlib.sha256(ser).digest()
        self.oid = base58.b58encode(hash).decode('utf-8')
        return self.oid


    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        order = {
            "m": {
                'c': self.symbol,
                'b': self.side.value == Side.BUY.value,
                'sz': f"{self.size:.8g}",
                'r': self.reduce_only,
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
    side: Optional[Side] = None
    nonce: Optional[str] = None

    def order_id(self) -> Optional[str]:
        return self.oid

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cx": {
                'c': self.symbol,
                'oid': self.oid
            }
        }


@dataclass
class CancelAll:
    """Cancel all orders for symbol or across symbols"""
    symbols: List[str]
    nonce: Optional[str] = None

    def order_id(self) -> Optional[str]:
        return None

    def to_api(self) -> Dict:
        """Convert to API format with compact field names"""
        return {
            "cxa": {
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

##
## Unit Tests
##
def _limitorder_id():
    order = LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=68000.0,
        size=0.001,
        reduce_only=False,
        time_in_force=TimeInForce.ALO,
        nonce=1772569595613073,
        pubkey="2bZfxVQtWdd8qAWJ4Xyq43cnej9zqMNyuh7HHxTNan8j"
    )
    oid = order.order_id()
    assert oid == "JBHReFLFMA4suv5qs7KTSfho5bFTkQRU8aQ4NYqyhuoJ"

if __name__ == "__main__":
    _limitorder_id()
