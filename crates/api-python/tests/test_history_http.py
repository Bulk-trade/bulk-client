import importlib.util
import json
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
HistoryBackfillStatus = history.HistoryBackfillStatus
HistoryFill = history.HistoryFill
HistoryHttpError = history.HistoryHttpError
HistoryPage = history.HistoryPage
HistoryPageInfo = history.HistoryPageInfo
RiskEvent = history.RiskEvent
RiskEventType = history.RiskEventType
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
    def __init__(self, status_code, payload=None, content=None):
        self.status_code = status_code
        self._payload = payload
        self.content = json.dumps(payload).encode() if content is None else content

    def json(self):
        if self._payload is None:
            raise ValueError("response is not JSON")
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
        "eventType": "risk_vault",
        "marginPrior": 10.0,
        "marginAfter": 2.0,
        "reason": "maintenance margin",
        "iso": True,
        "isoPubkey": PUBKEY,
        "slot": U64_MAX - 15,
        "timestamp": U64_MAX - 14,
        "sequence": U64_MAX - 13,
    }


class HistoryPageEnvelopeTests(unittest.TestCase):
    def test_rejects_missing_next_cursor(self):
        metadata = page(fill_row())["page"]
        metadata["hasMore"] = False
        del metadata["nextCursor"]

        with self.assertRaisesRegex(ValueError, "nextCursor"):
            HistoryPageInfo.from_api(metadata)

    def test_rejects_invalid_primitives_and_u64_values(self):
        cases = (
            ("nextCursor", 1),
            ("hasMore", 1),
            ("asOfSlot", True),
            ("asOfSlot", -1),
            ("startSlot", U64_MAX + 1),
            ("endSlot", 1.0),
            ("coverage", True),
            ("minAvailableSlot", False),
            ("backfillStatus", 1),
        )

        for field, value in cases:
            with self.subTest(field=field, value=value):
                metadata = page(fill_row())["page"]
                metadata[field] = value
                with self.assertRaisesRegex(ValueError, field):
                    HistoryPageInfo.from_api(metadata)

    def test_rejects_inconsistent_cursor_and_slot_bounds(self):
        cases = (
            {"hasMore": True, "nextCursor": None},
            {"hasMore": True, "nextCursor": ""},
            {"hasMore": False, "nextCursor": "next_page"},
            {"hasMore": False, "nextCursor": ""},
            {"startSlot": 11, "endSlot": 10},
            {"endSlot": 11, "asOfSlot": 10},
            {
                "minAvailableSlot": 11,
                "startSlot": 0,
                "endSlot": 10,
                "asOfSlot": 10,
            },
        )

        for updates in cases:
            with self.subTest(updates=updates):
                metadata = page(fill_row())["page"]
                metadata.update(updates)
                with self.assertRaises(ValueError):
                    HistoryPageInfo.from_api(metadata)

    def test_accepts_unknown_page_below_retained_floor(self):
        metadata = page(fill_row())["page"]
        metadata.update(
            {
                "nextCursor": None,
                "hasMore": False,
                "asOfSlot": 20,
                "startSlot": 20,
                "endSlot": 20,
                "coverage": "unknown",
                "minAvailableSlot": 50,
            }
        )

        result = HistoryPageInfo.from_api(metadata)

        self.assertEqual(result.coverage, HistoryCoverageStatus.UNKNOWN)
        self.assertEqual(result.min_available_slot, 50)

    def test_rejects_malformed_envelope_before_decoding_rows(self):
        class FailIfDecoded:
            @classmethod
            def from_api(cls, _data):
                raise AssertionError("row decoding must not run")

        malformed = (
            None,
            [],
            {"data": {}, "page": page(fill_row())["page"]},
            {"data": [fill_row()], "page": []},
            {
                "data": [fill_row()],
                "page": {**page(fill_row())["page"], "hasMore": 1},
            },
        )

        for payload in malformed:
            with self.subTest(payload=payload):
                with self.assertRaises(ValueError):
                    HistoryPage.from_api(payload, FailIfDecoded)


class HistoryHttpTests(unittest.TestCase):
    def setUp(self):
        self.client = BulkHttpClient(base_url="https://example.test/api/v1")

    @patch.object(http.requests, "get")
    @patch.object(http.requests, "post")
    def test_history_first_page_posts_exact_camel_case_body_and_preserves_u64(self, post, get):
        get.return_value = post.return_value = FakeResponse(200, page(fill_row()))

        result = self.client.get_fills_page(
            PUBKEY,
            limit=5000,
            start_slot=9_007_199_254_740_993,
            end_slot=U64_MAX,
        )

        post.assert_called_once_with(
            "https://example.test/api/v1/account",
            json={
                "type": "fills",
                "user": PUBKEY,
                "limit": 5000,
                "startSlot": 9_007_199_254_740_993,
                "endSlot": U64_MAX,
            },
            timeout=10,
        )
        get.assert_not_called()
        self.assertIsInstance(result.data[0], HistoryFill)
        self.assertEqual(result.data[0].slot, U64_MAX)
        self.assertEqual(result.page.coverage, HistoryCoverageStatus.COMPLETE)

    @patch.object(http.requests, "get")
    @patch.object(http.requests, "post")
    def test_history_continuation_posts_only_limit_and_cursor_without_auto_follow(self, post, get):
        get.return_value = post.return_value = FakeResponse(200, page(fill_row()))

        result = self.client.get_fills_page(PUBKEY, limit=17, cursor="next_page")

        post.assert_called_once_with(
            "https://example.test/api/v1/account",
            json={
                "type": "fills",
                "user": PUBKEY,
                "limit": 17,
                "cursor": "next_page",
            },
            timeout=10,
        )
        get.assert_not_called()
        self.assertTrue(result.page.has_more)
        self.assertEqual(result.page.next_cursor, "next_page")

    @patch.object(http.requests, "get")
    @patch.object(http.requests, "post")
    def test_history_all_six_methods_post_exact_types_and_decode_distinct_rows(self, post, get):
        cases = [
            ("get_fills_page", "fills", fill_row(), HistoryFill),
            ("get_positions_page", "positions", position_row(), ClosedPosition),
            ("get_funding_page", "fundingHistory", funding_row(), FundingPayment),
            ("get_orders_page", "orderHistory", order_row(), TerminalOrder),
            ("get_activity_page", "activityHistory", activity_row(), AccountActivity),
            ("get_risk_page", "riskHistory", risk_row(), RiskEvent),
        ]

        for method, request_type, row, row_type in cases:
            with self.subTest(request_type=request_type):
                get.reset_mock()
                post.reset_mock()
                get.return_value = post.return_value = FakeResponse(200, page(row))
                result = getattr(self.client, method)(PUBKEY)
                post.assert_called_once_with(
                    "https://example.test/api/v1/account",
                    json={"type": request_type, "user": PUBKEY},
                    timeout=10,
                )
                get.assert_not_called()
                self.assertIsInstance(result.data[0], row_type)
                if request_type == "riskHistory":
                    self.assertEqual(result.data[0].event_type, RiskEventType.RISK_VAULT)

    @patch.object(http.requests, "post")
    def test_history_risk_rejects_undocumented_event_type(self, post):
        row = risk_row()
        row["eventType"] = "unknown"
        post.return_value = FakeResponse(200, page(row))

        with self.assertRaisesRegex(ValueError, "unknown risk event type"):
            self.client.get_risk_page(PUBKEY)

    @patch.object(http.requests, "post")
    def test_history_backfill_status_is_optional_and_strict(self, post):
        post.return_value = FakeResponse(200, page(fill_row()))
        self.assertIsNone(self.client.get_fills_page(PUBKEY).page.backfill_status)

        pending = page(fill_row())
        pending["page"]["backfillStatus"] = "pending"
        post.return_value = FakeResponse(200, pending)
        self.assertEqual(
            self.client.get_fills_page(PUBKEY).page.backfill_status,
            HistoryBackfillStatus.PENDING,
        )

        unknown = page(fill_row())
        unknown["page"]["backfillStatus"] = "complete"
        post.return_value = FakeResponse(200, unknown)
        with self.assertRaises(ValueError):
            self.client.get_fills_page(PUBKEY)

    @patch.object(http.requests, "post")
    def test_history_non_success_preserves_structured_status_and_body(self, post):
        body = {
            "error": {
                "code": "CURSOR_EXPIRED",
                "message": "history changed",
            }
        }
        post.return_value = FakeResponse(410, body)

        with self.assertRaises(HistoryHttpError) as raised:
            self.client.get_risk_page(PUBKEY)

        self.assertEqual(raised.exception.status, 410)
        self.assertEqual(raised.exception.body.error.code, "CURSOR_EXPIRED")
        self.assertEqual(raised.exception.body.error.message, "history changed")

    @patch.object(http.requests, "post")
    def test_history_non_contract_errors_preserve_status_with_bounded_fallback(self, post):
        for status, content in (
            (502, b""),
            (503, b"<html>" + b"x" * (16 * 1024) + b"</html>"),
            (418, b'{"error":"upstream overloaded"}'),
        ):
            with self.subTest(status=status):
                post.return_value = FakeResponse(status, content=content)

                with self.assertRaises(HistoryHttpError) as raised:
                    self.client.get_fills_page(PUBKEY)

                self.assertEqual(raised.exception.status, status)
                self.assertEqual(
                    raised.exception.body.error.code,
                    "HISTORY_HTTP_ERROR",
                )
                self.assertIn(str(status), raised.exception.body.error.message)
                self.assertLessEqual(len(raised.exception.body.error.message), 256)

    @patch.object(http.requests, "post")
    def test_history_order_trigger_is_strict_and_typed(self, post):
        post.return_value = FakeResponse(200, page(order_row()))

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
        post.return_value = FakeResponse(200, page(malformed))
        with self.assertRaises(KeyError):
            self.client.get_orders_page(PUBKEY)


if __name__ == "__main__":
    unittest.main()
