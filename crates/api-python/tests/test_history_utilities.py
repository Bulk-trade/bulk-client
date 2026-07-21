import importlib.util
import json
import sys
import tempfile
import types
import unittest
from importlib.machinery import SourceFileLoader
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
PUBKEY = "11111111111111111111111111111111"


def load_module(name, path, patches=None, loader=None):
    patches = patches or {}
    missing = object()
    previous = {key: sys.modules.get(key, missing) for key in patches}
    sys.modules.update(patches)
    loader = loader or SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        loader.exec_module(module)
    finally:
        for key, value in previous.items():
            if value is missing:
                sys.modules.pop(key, None)
            else:
                sys.modules[key] = value
    return module


history = load_module(
    "bulk_test_history_utilities",
    ROOT / "bulk_api/messages/history.py",
)
ClosedPosition = history.ClosedPosition
HistoryCoverageStatus = history.HistoryCoverageStatus
HistoryPage = history.HistoryPage
HistoryPageInfo = history.HistoryPageInfo


def load_utility(name):
    path = ROOT / "utilities" / name
    package = types.ModuleType("bulk_api")
    package.BulkHttpClient = object
    patches = {"bulk_api": package}
    if name == "pnl_from_fills":
        pandas = types.ModuleType("pandas")
        pandas.Timestamp = lambda value, unit: (value, unit)
        common = types.ModuleType("bulk_api.common")
        comparisons = types.ModuleType("bulk_api.common.comparisons")
        inventory = types.ModuleType("bulk_api.common.inventory")
        inventory.Inventory = object
        inventory.Position = object
        inventory.Pnl = object
        patches.update(
            {
                "pandas": pandas,
                "bulk_api.common": common,
                "bulk_api.common.comparisons": comparisons,
                "bulk_api.common.inventory": inventory,
            }
        )
    return load_module(
        f"bulk_test_{name}",
        path,
        patches,
        SourceFileLoader(f"bulk_test_{name}", str(path)),
    )


def position(sequence):
    return ClosedPosition(
        owner=PUBKEY,
        symbol="BTC-USD",
        quantity=1.0,
        max_quantity=1.0,
        total_volume=1.0,
        avg_open_price=90_000.0,
        avg_close_price=100_000.0,
        realized_pnl=10_000.0,
        fees=1.0,
        funding=0.0,
        open_time=1,
        close_time=2,
        close_reason="normal",
        iso=False,
        iso_pubkey=None,
        close_slot=100,
        sequence=sequence,
    )


def history_page(rows, cursor):
    return HistoryPage(
        data=rows,
        page=HistoryPageInfo(
            next_cursor=cursor,
            has_more=cursor is not None,
            as_of_slot=100,
            start_slot=1,
            end_slot=100,
            coverage=HistoryCoverageStatus.COMPLETE,
            min_available_slot=1,
        ),
    )


class PositionHistoryUtilityTests(unittest.TestCase):
    def test_position_history_is_public_and_fetches_only_enough_pages_for_n(self):
        utility = load_utility("position_history")
        calls = []
        clients = []

        class FakeClient:
            def __init__(self, **kwargs):
                clients.append(kwargs)
                self.pages = iter(
                    [
                        history_page([position(4), position(3)], "cursor-1"),
                        history_page([position(2)], "cursor-2"),
                    ]
                )

            def get_positions_page(self, user, **kwargs):
                calls.append((user, kwargs))
                return next(self.pages)

        with patch.object(utility, "BulkHttpClient", FakeClient):
            rows = utility.position_history(
                account=PUBKEY,
                url="https://example.test/api/v1",
                n=3,
            )

        self.assertEqual(clients, [{"base_url": "https://example.test/api/v1"}])
        self.assertEqual(
            calls,
            [
                (PUBKEY, {"limit": 3, "cursor": None}),
                (PUBKEY, {"limit": 1, "cursor": "cursor-1"}),
            ],
        )
        self.assertEqual([row.sequence for row in rows], [4, 3, 2])

    def test_position_history_stops_at_terminal_page_before_n(self):
        utility = load_utility("position_history")

        class FakeClient:
            def __init__(self, **_kwargs):
                pass

            def get_positions_page(self, _user, **_kwargs):
                return history_page([position(1)], None)

        with patch.object(utility, "BulkHttpClient", FakeClient):
            rows = utility.position_history(
                account=PUBKEY,
                url="https://example.test/api/v1",
                n=10,
            )

        self.assertEqual([row.sequence for row in rows], [1])


class PnlFromFillsUtilityTests(unittest.TestCase):
    def test_pnl_from_fills_reads_page_data_in_ascending_slot_sequence_order(self):
        utility = load_utility("pnl_from_fills")
        rows = [
            {"slot": 10, "sequence": 2, "isBuy": True, "amount": 1.0, "price": 3.0, "symbol": "BTC-USD", "timestamp": 3},
            {"slot": 9, "sequence": 7, "isBuy": False, "amount": 1.0, "price": 1.0, "symbol": "BTC-USD", "timestamp": 1},
            {"slot": 10, "sequence": 1, "isBuy": True, "amount": 1.0, "price": 2.0, "symbol": "BTC-USD", "timestamp": 2},
        ]
        payload = {
            "data": rows,
            "page": {
                "nextCursor": None,
                "hasMore": False,
                "asOfSlot": 10,
                "startSlot": 9,
                "endSlot": 10,
                "coverage": "complete",
                "minAvailableSlot": 9,
            },
        }

        with tempfile.NamedTemporaryFile("w", suffix=".json") as fixture:
            json.dump(payload, fixture)
            fixture.flush()
            fills = utility.parse_fills(fixture.name)

        self.assertEqual(
            [(fill["slot"], fill["sequence"]) for fill in fills],
            [(9, 7), (10, 1), (10, 2)],
        )


if __name__ == "__main__":
    unittest.main()
