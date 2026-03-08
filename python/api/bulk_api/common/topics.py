from enum import Enum


class Topic(Enum):
    """
    WebSocket topics

    Event types and their emitted objects:
    - "ticker": Ticker
    - "trades": List[Trade]
    - "l2_snapshot": L2Snapshot
    - "l2_delta": L2Delta
    - "candle": Candle
    - "account_snapshot": AccountSnapshot
    - "margin_update": MarginUpdate
    - "position_update": PositionUpdate
    - "fill": Fill
    - "leverage_update": List[LeverageSetting]
    - "order_state": OrderState
    """
    TICKER = "ticker"
    TRADES = "trades"
    L2SNAPSHOT = "l2_snapshot"
    L2DELTA = "l2_delta"
    CANDLE = "candle"
    ACCOUNT = "account_snapshot"
    MARGIN = "margin_update"
    POSITION = "position_update"
    FILL = "fill"
    LEVERAGE = "leverage_update"
    ORDER = "order_state"
    ERROR = "error"