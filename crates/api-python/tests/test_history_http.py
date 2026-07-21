import importlib.util
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]


def load_module(name, path, patches=None):
    patches = patches or {}
    missing = object()
    previous = {key: sys.modules.get(key, missing) for key in patches}
    sys.modules.update(patches)
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        for key, value in previous.items():
            if value is missing:
                sys.modules.pop(key, None)
            else:
                sys.modules[key] = value
    return module


history = load_module(
    "bulk_test_history",
    ROOT / "bulk_api/messages/history.py",
)
AccountActivity = history.AccountActivity
ClosedPosition = history.ClosedPosition
FundingPayment = history.FundingPayment
HistoryCoverageStatus = history.HistoryCoverageStatus
HistoryFill = history.HistoryFill
HistoryHttpError = history.HistoryHttpError
RiskEvent = history.RiskEvent
TerminalOrder = history.TerminalOrder
HistoryTrigger = history.HistoryTrigger


def load_http():
    common = types.ModuleType("bulk_api.common")
    common.TransactionSigner = object
    common.Side = object
    common.TimeInForce = object

    messages = types.ModuleType("bulk_api.messages")
    for name in (
        "ApproveBuilderCode",
        "LimitOrder",
        "CancelOrder",
        "MarketOrder",
        "CancelAll",
        "RevokeBuilderCode",
    ):
        setattr(messages, name, type(name, (), {}))
    for name in (
        "AccountActivity",
        "ClosedPosition",
        "FundingPayment",
        "HistoryErrorEnvelope",
        "HistoryFill",
        "HistoryHttpError",
        "HistoryPage",
        "RiskEvent",
        "TerminalOrder",
    ):
        setattr(messages, name, getattr(history, name))

    requests = types.ModuleType("requests")
    requests.get = None
    requests.post = None

    package = types.ModuleType("bulk_api")
    package.common = common
    package.messages = messages
    return load_module(
        "bulk_test_http",
        ROOT / "bulk_api/api/bulk_http.py",
        {
            "bulk_api": package,
            "bulk_api.common": common,
            "bulk_api.messages": messages,
            "requests": requests,
        },
    )


http = load_http()
BulkHttpClient = http.BulkHttpClient


PUBKEY = "11111111111111111111111111111111"
U64_MAX = 18_446_744_073_709_551_615


class FakeResponse:
    def __init__(self, status_code, payload):
        self.status_code = status_code
        self._payload = payload

    def json(self):
        return self._payload


def page(row):
    return {
        "data": [row],
        "page": {
            "nextCursor": "next_page",
            "hasMore": True,
            "asOfSlot": U64_MAX,
            "startSlot": 9_007_199_254_740_993,
            "endSlot": U64_MAX,
            "coverage": "complete",
            "minAvailableSlot": 9_007_199_254_740_993,
        },
    }


def fill_row():
    return {
        "maker": PUBKEY,
        "taker": PUBKEY,
        "orderIdMaker": PUBKEY,
        "orderIdTaker": PUBKEY,
        "isBuy": True,
        "symbol": "BTC-USD",
        "amount": 1.25,
        "price": 100_000.0,
        "makerFee": 1.0,
        "takerFee": 2.0,
        "fee": 1.0,
        "reasonCode": 3,
        "iso": True,
        "isoPubkey": PUBKEY,
        "reason": "matched",
        "counterpartyHint": "1111..1111",
        "slot": U64_MAX,
        "timestamp": U64_MAX - 1,
        "sequence": U64_MAX - 2,
    }


def position_row():
    return {
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "quantity": -1.25,
        "maxQuantity": -2.0,
        "totalVolume": 3.0,
        "avgOpenPrice": 90_000.0,
        "avgClosePrice": 100_000.0,
        "realizedPnl": 12_500.0,
        "fees": 12.0,
        "funding": -2.0,
        "openTime": U64_MAX - 10,
        "closeTime": U64_MAX - 9,
        "closeReason": "normal",
        "iso": True,
        "isoPubkey": PUBKEY,
        "closeSlot": U64_MAX - 8,
        "sequence": U64_MAX - 7,
    }


def funding_row():
    return {
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "size": -1.25,
        "payment": 3.5,
        "fundingRate": 0.0001,
        "markPrice": 100_000.0,
        "iso": True,
        "isoPubkey": PUBKEY,
        "slot": U64_MAX - 6,
        "timestamp": U64_MAX - 5,
        "sequence": U64_MAX - 4,
    }


def order_row():
    return {
        "orderId": PUBKEY,
        "symbol": "BTC-USD",
        "side": "buy",
        "orderType": "limit",
        "tif": "gtc",
        "price": 100_000.0,
        "vwap": 99_999.0,
        "originalSize": 1.25,
        "executedSize": 1.25,
        "reduceOnly": False,
        "status": "filled",
        "trigger": {
            "isAbove": True,
            "px": 101_000.0,
            "lim": 100_500.0,
            "oco": PUBKEY,
            "pxHi": 102_000.0,
            "limHi": 101_500.0,
            "trb": 250,
            "stb": 25,
        },
        "reason": "filled",
        "iso": True,
        "isoPubkey": PUBKEY,
        "slot": U64_MAX - 3,
        "timestamp": U64_MAX - 2,
        "sequence": U64_MAX - 1,
    }


def activity_row():
    return {
        "activityType": "transfer",
        "status": "settled",
        "from": PUBKEY,
        "to": PUBKEY,
        "symbol": "USDC",
        "amount": 25.0,
        "reason": "test",
        "iso": True,
        "isoPubkey": PUBKEY,
        "slot": U64_MAX - 12,
        "timestamp": U64_MAX - 11,
        "sequence": U64_MAX - 10,
    }


def risk_row():
    return {
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "isBuy": False,
        "amount": 0.5,
        "price": 80_000.0,
        "eventType": "liquidation",
        "marginPrior": 10.0,
        "marginAfter": 2.0,
        "reason": "maintenance margin",
        "iso": True,
        "isoPubkey": PUBKEY,
        "slot": U64_MAX - 15,
        "timestamp": U64_MAX - 14,
        "sequence": U64_MAX - 13,
    }


class HistoryHttpTests(unittest.TestCase):
    def setUp(self):
        self.client = BulkHttpClient(base_url="https://example.test/api/v1")

    @patch.object(http.requests, "get")
    def test_history_first_page_uses_exact_camel_case_params_and_preserves_u64(self, get):
        get.return_value = FakeResponse(200, page(fill_row()))

        result = self.client.get_fills_page(
            PUBKEY,
            limit=500,
            start_slot=9_007_199_254_740_993,
            end_slot=U64_MAX,
        )

        get.assert_called_once_with(
            f"https://example.test/api/v1/accounts/{PUBKEY}/history/fills",
            params={
                "limit": 500,
                "startSlot": 9_007_199_254_740_993,
                "endSlot": U64_MAX,
            },
            timeout=10,
        )
        self.assertIsInstance(result.data[0], HistoryFill)
        self.assertEqual(result.data[0].slot, U64_MAX)
        self.assertEqual(result.page.coverage, HistoryCoverageStatus.COMPLETE)

    @patch.object(http.requests, "get")
    def test_history_continuation_uses_only_limit_and_cursor_without_auto_follow(self, get):
        get.return_value = FakeResponse(200, page(fill_row()))

        result = self.client.get_fills_page(PUBKEY, limit=17, cursor="next_page")

        get.assert_called_once_with(
            f"https://example.test/api/v1/accounts/{PUBKEY}/history/fills",
            params={"limit": 17, "cursor": "next_page"},
            timeout=10,
        )
        self.assertTrue(result.page.has_more)
        self.assertEqual(result.page.next_cursor, "next_page")

    @patch.object(http.requests, "get")
    def test_history_all_six_methods_use_exact_paths_and_distinct_rows(self, get):
        cases = [
            ("get_fills_page", "fills", fill_row(), HistoryFill),
            ("get_positions_page", "positions", position_row(), ClosedPosition),
            ("get_funding_page", "funding", funding_row(), FundingPayment),
            ("get_orders_page", "orders", order_row(), TerminalOrder),
            ("get_activity_page", "activity", activity_row(), AccountActivity),
            ("get_risk_page", "risk", risk_row(), RiskEvent),
        ]

        for method, kind, row, row_type in cases:
            with self.subTest(kind=kind):
                get.reset_mock()
                get.return_value = FakeResponse(200, page(row))
                result = getattr(self.client, method)(PUBKEY)
                get.assert_called_once_with(
                    f"https://example.test/api/v1/accounts/{PUBKEY}/history/{kind}",
                    params={},
                    timeout=10,
                )
                self.assertIsInstance(result.data[0], row_type)

    @patch.object(http.requests, "get")
    def test_history_non_success_preserves_structured_status_and_body(self, get):
        body = {
            "error": {
                "code": "CURSOR_EXPIRED",
                "message": "history changed",
            }
        }
        get.return_value = FakeResponse(410, body)

        with self.assertRaises(HistoryHttpError) as raised:
            self.client.get_risk_page(PUBKEY)

        self.assertEqual(raised.exception.status, 410)
        self.assertEqual(raised.exception.body.error.code, "CURSOR_EXPIRED")
        self.assertEqual(raised.exception.body.error.message, "history changed")

    @patch.object(http.requests, "get")
    def test_history_order_trigger_is_strict_and_typed(self, get):
        get.return_value = FakeResponse(200, page(order_row()))

        order = self.client.get_orders_page(PUBKEY).data[0]

        self.assertIsInstance(order.trigger, HistoryTrigger)
        self.assertEqual(order.trigger.is_above, True)
        self.assertEqual(order.trigger.px, 101_000.0)
        self.assertEqual(order.trigger.lim, 100_500.0)
        self.assertEqual(order.trigger.oco, PUBKEY)
        self.assertEqual(order.trigger.px_hi, 102_000.0)
        self.assertEqual(order.trigger.lim_hi, 101_500.0)
        self.assertEqual(order.trigger.trail_bps, 250)
        self.assertEqual(order.trigger.step_bps, 25)

        malformed = order_row()
        del malformed["trigger"]["px"]
        get.return_value = FakeResponse(200, page(malformed))
        with self.assertRaises(KeyError):
            self.client.get_orders_page(PUBKEY)


if __name__ == "__main__":
    unittest.main()
