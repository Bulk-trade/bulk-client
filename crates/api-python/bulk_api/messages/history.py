from dataclasses import dataclass
from enum import Enum
from typing import Any, Dict, Generic, List, Optional, Type, TypeVar


class HistoryCoverageStatus(str, Enum):
    COMPLETE = "complete"
    PARTIAL = "partial"
    UNKNOWN = "unknown"


@dataclass
class HistoryPageInfo:
    next_cursor: Optional[str]
    has_more: bool
    as_of_slot: int
    start_slot: int
    end_slot: int
    coverage: HistoryCoverageStatus
    min_available_slot: Optional[int]

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "HistoryPageInfo":
        return cls(
            next_cursor=data.get("nextCursor"),
            has_more=data["hasMore"],
            as_of_slot=data["asOfSlot"],
            start_slot=data["startSlot"],
            end_slot=data["endSlot"],
            coverage=HistoryCoverageStatus(data["coverage"]),
            min_available_slot=data.get("minAvailableSlot"),
        )


HistoryRow = TypeVar("HistoryRow")


@dataclass
class HistoryPage(Generic[HistoryRow]):
    data: List[HistoryRow]
    page: HistoryPageInfo

    @classmethod
    def from_api(
        cls,
        payload: Dict[str, Any],
        row_type: Type[HistoryRow],
    ) -> "HistoryPage[HistoryRow]":
        return cls(
            data=[row_type.from_api(row) for row in payload["data"]],
            page=HistoryPageInfo.from_api(payload["page"]),
        )


@dataclass
class HistoryErrorBody:
    code: str
    message: str

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "HistoryErrorBody":
        return cls(code=data["code"], message=data["message"])


@dataclass
class HistoryErrorEnvelope:
    error: HistoryErrorBody

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "HistoryErrorEnvelope":
        return cls(error=HistoryErrorBody.from_api(data["error"]))


class HistoryHttpError(Exception):
    def __init__(self, status: int, body: HistoryErrorEnvelope):
        self.status = status
        self.body = body
        super().__init__(f"history API returned {status}: {body.error.code}: {body.error.message}")


@dataclass
class HistoryFill:
    maker: str
    taker: str
    order_id_maker: str
    order_id_taker: str
    is_buy: bool
    symbol: str
    amount: float
    price: float
    maker_fee: float
    taker_fee: float
    fee: float
    reason_code: int
    iso: bool
    iso_pubkey: Optional[str]
    reason: Optional[str]
    counterparty_hint: Optional[str]
    slot: int
    timestamp: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "HistoryFill":
        return cls(
            maker=data["maker"],
            taker=data["taker"],
            order_id_maker=data["orderIdMaker"],
            order_id_taker=data["orderIdTaker"],
            is_buy=data["isBuy"],
            symbol=data["symbol"],
            amount=data["amount"],
            price=data["price"],
            maker_fee=data["makerFee"],
            taker_fee=data["takerFee"],
            fee=data["fee"],
            reason_code=data["reasonCode"],
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            reason=data.get("reason"),
            counterparty_hint=data.get("counterpartyHint"),
            slot=data["slot"],
            timestamp=data["timestamp"],
            sequence=data["sequence"],
        )


@dataclass
class ClosedPosition:
    owner: str
    symbol: str
    quantity: float
    max_quantity: float
    total_volume: float
    avg_open_price: float
    avg_close_price: float
    realized_pnl: float
    fees: float
    funding: float
    open_time: int
    close_time: int
    close_reason: str
    iso: bool
    iso_pubkey: Optional[str]
    close_slot: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "ClosedPosition":
        return cls(
            owner=data["owner"],
            symbol=data["symbol"],
            quantity=data.get("quantity", 0.0),
            max_quantity=data.get("maxQuantity", 0.0),
            total_volume=data["totalVolume"],
            avg_open_price=data["avgOpenPrice"],
            avg_close_price=data["avgClosePrice"],
            realized_pnl=data["realizedPnl"],
            fees=data["fees"],
            funding=data["funding"],
            open_time=data["openTime"],
            close_time=data["closeTime"],
            close_reason=data["closeReason"],
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            close_slot=data["closeSlot"],
            sequence=data["sequence"],
        )


@dataclass
class FundingPayment:
    owner: str
    symbol: str
    size: float
    payment: float
    funding_rate: float
    mark_price: float
    iso: bool
    iso_pubkey: Optional[str]
    slot: int
    timestamp: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "FundingPayment":
        return cls(
            owner=data["owner"],
            symbol=data["symbol"],
            size=data["size"],
            payment=data["payment"],
            funding_rate=data["fundingRate"],
            mark_price=data["markPrice"],
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            slot=data["slot"],
            timestamp=data["timestamp"],
            sequence=data["sequence"],
        )


@dataclass
class HistoryTrigger:
    is_above: Optional[bool]
    px: float
    lim: Optional[float]
    oco: Optional[str]
    px_hi: Optional[float]
    lim_hi: Optional[float]
    trail_bps: Optional[int]
    step_bps: Optional[int]

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "HistoryTrigger":
        return cls(
            is_above=data.get("isAbove"),
            px=data["px"],
            lim=data.get("lim"),
            oco=data.get("oco"),
            px_hi=data.get("pxHi"),
            lim_hi=data.get("limHi"),
            trail_bps=data.get("trb"),
            step_bps=data.get("stb"),
        )


@dataclass
class TerminalOrder:
    order_id: str
    symbol: str
    side: str
    order_type: str
    tif: str
    price: float
    vwap: float
    original_size: float
    executed_size: float
    reduce_only: bool
    status: str
    trigger: Optional[HistoryTrigger]
    reason: Optional[str]
    iso: bool
    iso_pubkey: Optional[str]
    slot: int
    timestamp: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "TerminalOrder":
        return cls(
            order_id=data["orderId"],
            symbol=data["symbol"],
            side=data["side"],
            order_type=data["orderType"],
            tif=data["tif"],
            price=data["price"],
            vwap=data["vwap"],
            original_size=data["originalSize"],
            executed_size=data["executedSize"],
            reduce_only=data["reduceOnly"],
            status=data["status"],
            trigger=(
                HistoryTrigger.from_api(data["trigger"])
                if data.get("trigger") is not None
                else None
            ),
            reason=data.get("reason"),
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            slot=data["slot"],
            timestamp=data["timestamp"],
            sequence=data["sequence"],
        )


@dataclass
class AccountActivity:
    activity_type: str
    status: str
    from_: str
    to: str
    symbol: str
    amount: float
    reason: Optional[str]
    iso: bool
    iso_pubkey: Optional[str]
    slot: int
    timestamp: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "AccountActivity":
        return cls(
            activity_type=data["activityType"],
            status=data["status"],
            from_=data["from"],
            to=data["to"],
            symbol=data["symbol"],
            amount=data["amount"],
            reason=data.get("reason"),
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            slot=data["slot"],
            timestamp=data["timestamp"],
            sequence=data["sequence"],
        )


class RiskEventType(str, Enum):
    LIQUIDATION = "liquidation"
    ADL = "adl"
    RISK_VAULT = "risk_vault"


@dataclass
class RiskEvent:
    owner: str
    symbol: str
    is_buy: bool
    amount: float
    price: float
    event_type: RiskEventType
    margin_prior: Optional[float]
    margin_after: Optional[float]
    reason: Optional[str]
    iso: bool
    iso_pubkey: Optional[str]
    slot: int
    timestamp: int
    sequence: int

    @classmethod
    def from_api(cls, data: Dict[str, Any]) -> "RiskEvent":
        try:
            event_type = RiskEventType(data["eventType"])
        except ValueError as error:
            raise ValueError(f"unknown risk event type: {data['eventType']}") from error
        return cls(
            owner=data["owner"],
            symbol=data["symbol"],
            is_buy=data["isBuy"],
            amount=data["amount"],
            price=data["price"],
            event_type=event_type,
            margin_prior=data.get("marginPrior"),
            margin_after=data.get("marginAfter"),
            reason=data.get("reason"),
            iso=data.get("iso", False),
            iso_pubkey=data.get("isoPubkey"),
            slot=data["slot"],
            timestamp=data["timestamp"],
            sequence=data["sequence"],
        )
