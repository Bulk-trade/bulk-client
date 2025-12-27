import numpy as np
import pandas as pd

from .comparisons import *

def is_dollar_equiv(instrument: str) -> bool:
    """
    Determine if a given instrument is dollar equivalent.  For the moment
    we assume such an instrument name starts with USD
    """
    return instrument.startswith("USD")


class Position:
    """
    Representation of a position
    """
    __slots__ = ('instrument', 'amount', 'vwap', 'pnl', 'mm', 'liqprice')

    def __init__(self, instrument: str, amount: float, vwap: float):
        self.instrument = instrument
        self.amount = amount
        self.vwap = vwap
        self.pnl = 0.0
        self.mm = 0.0
        self.liqprice = 0.0

    @property
    def side(self):
        return np.sign(self.amount)

    def is_dollar_equivalent(self):
        """For speed we assume stables we use start with USD"""
        return self.instrument.startswith("USD")

    def pv (self, prices):
        instrument = self.instrument

        if self.is_dollar_equivalent():
            self.pnl = self.amount
        else:
            price = prices[instrument] if not isinstance(prices,float) else prices
            self.pnl = self.amount * (price - self.vwap)
        return self.pnl

    def to_dict(self):
        return { 'id': self.instrument, 'amount': self.amount, 'price': self.vwap, 'pv': self.pnl }


class Pnl:
    """
    Representation of P&L
    """
    __slots__ = ('realized', 'unrealized')

    def __init__(self, realized: float, unrealized: float):
        self.realized = realized
        self.unrealized = unrealized

    @property
    def net(self):
        return self.realized + self.unrealized


class Inventory:
    """
    Track live trades, realized, P&L, margin, maintenance margin, etc
    """
    def __init__(self, stable = "USDC"):
        self.stable = stable
        self.realized = 0.0
        self.unrealized = 0.0
        self.positions = {}

        self.fees = 0.0
        self.funding = 0.0


    def position_for(self, instrument: str):
        """
        Get position associated with instrument
        """
        pos = self.positions.get(instrument)
        if not (pos is None):
            return pos
        else:
            vwap = 1.0 if is_dollar_equiv(instrument) else 0.0
            self.positions[instrument] = Position(instrument, 0.0, vwap)
            return self.positions[instrument]

    def add_cash (self, amount: float, symbol: str = "USDC"):
        """
        Add a cash
        """
        cash = self.position_for(symbol)
        cash.amount += amount

    def update_account(self, info):
        """
        Update with latest account info

        :param info: account info
        """
        pass

    def traded(self, instrument: str, side: float, amount: float, price: float):
        """
        Add trade to inventory
        """
        position = self.position_for(instrument)
        pamount = position.amount

        if isZero(pamount):
            # new position
            position.amount = side * amount
            position.vwap = price

        elif (side * pamount) < 0:
            # check whether is a position reduction
            liquidated = min(np.abs(pamount), amount)
            cash = self.position_for(self.stable)
            cash.amount += position.side * liquidated * (price - position.vwap)

            if GT (amount, liquidated):
                position.amount = side * (amount - liquidated)
                position.vwap = price
            else:
                position.amount += side * amount
        else:
            nvwap = (np.abs(pamount) * position.vwap + amount * price) / (np.abs(pamount) + amount)
            position.amount += side * amount
            position.vwap = nvwap


    def pv (self, prices):
        """
        Compute PV of inventory given latest prices
        """
        realized = 0.0
        unrealized = 0.0

        for id, position in self.positions.items():
            if is_dollar_equiv(id):
                realized += position.amount
            else:
                unrealized += position.pv (prices)

        return Pnl (realized, unrealized)


    def summary(self):
        """
        Summary of positions
        """
        return [x.to_dict() for id, x in self.positions.items()]

    @staticmethod
    def pnl_for (trades: pd.DataFrame, prices: pd.DataFrame, type = "net"):
        """
        Process a dataframe of trades and generate rolling P&L for each instrument + combined
        """
        if "stamp" in trades.columns:
            trades = trades.set_index("stamp")
        if "stamp" in prices.columns:
            prices = prices.set_index("stamp")

        prices = prices.drop_duplicates()
        times_prices = prices.index
        times_trades = trades.index

        def equityFor (trades: pd.DataFrame):
            vrealized = []
            vunrealized = []
            inventory = Inventory()

            itrade = 0
            for t in range(prices.shape[0]):
                if t < prices.shape[0] - 1:
                    Tmkt = times_prices[t]
                else:
                    Tmkt = max(times_prices[t], times_trades[-1])

                # process any trades prior to current price set
                while itrade < trades.shape[0] and times_trades[itrade] <= Tmkt:
                    trade = trades.iloc[itrade]
                    instrument = trade["instrument"]
                    side = -1 if trade["side"] == "Sell" else +1
                    amount = trade["amount"]
                    price = trade["price"]

                    inventory.traded(instrument, side, amount, price)
                    itrade += 1

                pnl = inventory.pv(prices.iloc[t])
                vrealized.append (pnl.realized)
                vunrealized.append (pnl.unrealized)

            realized = pd.Series(vrealized, index=times_prices)
            unrealized = pd.Series(vunrealized, index=times_prices)
            if type == "net":
                return realized + unrealized
            elif type == "realized":
                return realized
            elif type == "unrealized":
                return unrealized
            else:
                raise Exception(f"unknown P&L type: {type}")

        instruments = trades.instrument.unique()
        equity = {}
        equity["*"] = equityFor(trades)
        for id in instruments:
            sub = trades.loc[trades.instrument == id]
            pnl = equityFor(sub)
            equity[id] = pnl

        return pd.DataFrame(equity, index=times_prices)
