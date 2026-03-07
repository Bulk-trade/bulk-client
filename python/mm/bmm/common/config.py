from typing import List, Set

import json5
import os
from dataclasses import dataclass


@dataclass
class MMConfig:
    """Market maker configuration"""
    # Symbol configuration
    coin: str

    # Connection configuration
    bulk_ws_url: str
    private_key: str

    # Order size per chunk
    chunk_size: float
    # Maximum number of new orders per level
    max_new_orders: int
    # Maximum number of price levels to replicate
    max_price_levels: int

    # tick resolution (in price) for the 1st K levels
    fine_tick: float
    # tick resolution (in price) for the deeper levels
    coarse_tick: float
    # max depth (bps)
    max_depth: float
    # maximum total # of live orders
    max_live_orders: int


    # Update frequency (seconds between syncs)
    sync_interval: float
    # Seconds between monitoring checks
    monitor_interval: float

    # Risk management
    max_inventory_notional: float
    inventory_close_fraction: float
    inventory_close_depth: float

    # Binance monitoring (seconds without update before reconnect)
    binance_timeout: float

    # Logging
    log_level: str
    # what to log: ["STATE", "FILL", "POSITION"]
    to_log: Set[str]

    def bulk_symbol(self) -> str:
        """symbol for bulk market"""
        return f"{self.coin}-USD"

    def binance_symbol(self) -> str:
        """symbol for binance market"""
        return f"{self.coin}USDT"

    @classmethod
    def load(cls, configfile: str) -> 'MMConfig':
        """
        Load config from file
        """
        with open(configfile, "r") as f:
            config = json5.load(f)
            return cls(
                coin=config.get("coin"),
                bulk_ws_url=config.get("bulk_ws_url", "wss://exchange-wss.bulk.trade"),
                private_key=None,
                chunk_size=config.get("chunk_size", 0.25),
                max_new_orders=config.get("max_new_orders", 5),
                max_price_levels=config.get("max_price_levels", 1000),
                fine_tick=config.get("fine_tick", 0.25),
                coarse_tick=config.get("coarse_tick", 5.0),
                max_depth=config.get("max_depth", 1000.0),
                max_live_orders=config.get("max_live_orders", 8000),
                sync_interval=config.get("sync_interval", 1.0),
                monitor_interval=config.get("monitor_interval", 1.0),
                max_inventory_notional=config.get("max_inventory_notional", 250000.0),
                inventory_close_fraction=config.get("inventory_close_fraction", 0.25),
                inventory_close_depth=config.get("inventory_close_depth", 50.0),
                binance_timeout=config.get("binance_timeout", 60.0),
                log_level=config.get("log_level", "INFO"),
                to_log=set(config.get("to_log", ["FILL", "POSITION"]))
            )