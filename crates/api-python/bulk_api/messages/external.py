"""SDK V13 external router messages. Timestamps are Unix nanoseconds."""
from dataclasses import dataclass, asdict
from typing import Optional


def _side(value):
    name = getattr(value, "name", value)
    return {"BUY": "Buy", "SELL": "Sell", "Buy": "Buy", "Sell": "Sell"}[name]


@dataclass
class ExternalFill:
    """External execution; account is taker, and fees are denominated in USD."""
    stamp: int
    account: str
    venue: str
    order_id: str
    side: str
    symbol: str
    amount: float
    price: float
    fees: float

    def to_api(self):
        report = asdict(self)
        report["side"] = _side(self.side)
        return {"externalFill": report}


@dataclass
class ExternalOrderState:
    """External lifecycle report, published by the executor as a normal IOC order state.

    status uses SDK wire names such as working, partiallyFilled, filled or cancelledIOC.
    The current SDK report codec requires positive price, vwap, amount and amount_done.
    """
    stamp: int
    account: str
    order_id: str
    side: str
    symbol: str
    price: float
    vwap: float
    amount: float
    amount_done: float
    status: str
    reason: Optional[str] = None

    def to_api(self):
        report = asdict(self)
        report["side"] = _side(self.side)
        return {"externalOrderState": report}


@dataclass
class OrderRequest:
    """Outbound order received on the global orderRequests topic; order_id is base58."""
    stamp: int
    order_id: str
    symbol: str
    side: str
    price: float
    size: float

    @classmethod
    def from_api(cls, data):
        return cls(data["stamp"], data["orderId"], data["symbol"], data["side"], data["price"], data["size"])
