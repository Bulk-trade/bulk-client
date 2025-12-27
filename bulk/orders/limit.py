import time
from typing import Optional

from bulk.common import OrderStatus, TimeInForce, Side, GE


class LimitOrder:
    """
    Representation of a limit order
    """
    __slots__ = ('oid', 'cid','side', 'instrument', 'amount', 'limit', 'amount_done', 'vwap', 'tif', 'reduce', 'status')

    def __init__(
        self,
        instrument: str,
        side: Side,
        amount: float,
        price: float,
        tif: TimeInForce = TimeInForce.GTC,
        reduce_only: bool = False,
        orderid: Optional[int] = None
    ):
        """
        Create limit order

        :param instrument: symbol name (for example 'BTC-USD')
        :param side: side of order (buy or sell)
        :param amount: amount in coin
        :param price: limit price
        :param tif: GTC, IOC, ALO
        :param reduce_only: if true, order only reduces position
        :param orderid: optional orderid
        """
        self.instrument = instrument
        self.side = side
        self.amount = amount
        self.limit = price
        self.amount_done = 0.0
        self.vwap = 0.0
        self.status = OrderStatus.NONE
        self.tif = tif
        self.reduce = reduce_only
        self.cid = int(time.time_ns()) if orderid is None else orderid
        self.oid = None

    def traded(self, amount: float, price: float) -> float:
        """Adjust order with a fill"""
        self.vwap = (self.amount_done * self.vwap + amount * price) / (self.amount_done + amount)
        self.amount_done += amount

    def pv(self, prices: dict) -> float:
        """order P&L in qccy terms"""
        px = prices[self.instrument]
        cdir = -1.0 if self.side == Side.SELL else 1.0
        cdir * self.amount * (px - self.vwap)

    def is_terminal(self) -> bool:
        """Determine if the order is terminal or not"""
        return self.status.is_terminal() or GE(self.amount_done, self.amount)

    def update(self, oid: str, status: OrderStatus):
        """Update order status"""
        self.oid = oid
        self.status = status

    def to_message(self):
        return {
            "c": self.instrument,
            "b": self.amount >= 0,
            "px": self.amount,
            "sz": abs(self.amount),
            "r": self.reduce,
            "t": {
                "limit": { "tif": str(self.tif) },
            }
        }