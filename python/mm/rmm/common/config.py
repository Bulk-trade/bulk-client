from typing import List, Set

import json5
import os
from dataclasses import dataclass


@dataclass
class RMMConfig:
    """Performance Testing Market maker configuration"""
    # Symbol configuration
    coin: str

    # Connection configuration
    bulk_ws_url: str
    private_key: str

    # base price
    price: float
    # mean reversion constant (for example 0.0001)
    kappa: float
    # standard deviation (for example 0.01)
    sigma: float

    # spreads to offer at
    spread: float
    # base size of inner level
    size: float
    # depth of book on each side (for example 500)
    depth: int
    # orders per level (for example 20)
    order_per_level: int
    # bundle chunking
    chunksize: int

    # frequency in seconds (for example 0.010)
    frequency: float
    # priming period before adding orders
    priming: int

    def bulk_symbol(self) -> str:
        """symbol for bulk market"""
        return f"{self.coin}-USD"

    @classmethod
    def load(cls, configfile: str) -> 'RMMConfig':
        """
        Load config from file
        """
        with open(configfile, "r") as f:
            config = json5.load(f)
            return cls(
                coin=config.get("coin"),
                bulk_ws_url=config.get("bulk_ws_url", "wss://exchange-wss.bulk.trade"),
                private_key=None,
                price=float(config.get("price", 100000.0)),
                kappa=float(config.get("kappa", 0.0001)),
                sigma=float(config.get("sigma", 0.01)),
                spread=float(config.get("spread", 0.10)),
                size=float(config.get("size", 0.5)),
                depth=int(config.get("depth", 500)),
                order_per_level=int(config.get("order_per_level", 20)),
                chunksize=int(config.get("chunksize", 500)),
                frequency=float(config.get("frequency", 0.010)),
                priming=int(config.get("priming", 100))
            )