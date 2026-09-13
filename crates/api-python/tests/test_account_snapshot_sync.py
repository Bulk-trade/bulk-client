import unittest

from bulk_api.api.bulk_ws import BulkWebSocketClient
from bulk_api.common import Side
from bulk_api.common.inventory import Inventory
from bulk_api.messages.account import OrderState


FULL_ORDER = {
    "timestamp": 7, "symbol": "BTC-USD", "orderId": "sell-1",
    "status": "resting", "orderType": "limit", "price": 100.0,
    "vwap": 100.0, "size": -2.0, "filledSize": 1.0, "originalSize": -3.0,
    "maker": True, "reduceOnly": True, "tif": "gtc",
}
SHORT_ORDER = {
    "ts": 7, "sym": "BTC-USD", "oid": "sell-1",
    "status": "resting", "ot": "limit", "px": 100.0,
    "vwap": 100.0, "sz": -2.0, "fillSz": 1.0, "origSz": -3.0,
    "mk": True, "r": True, "tif": "gtc",
}


class AccountSnapshotSync(unittest.IsolatedAsyncioTestCase):
    def test_both_order_shapes_preserve_sell_fields(self):
        full = OrderState.from_api(FULL_ORDER)
        self.assertEqual(full, OrderState.from_api(SHORT_ORDER))
        self.assertEqual(full.order_id, "sell-1")
        self.assertEqual(full.side, Side.SELL)
        self.assertEqual(full.get_side(), Side.SELL)
        self.assertEqual(full.amount_remaining(), 2.0)
        self.assertEqual(full.size, 2.0)
        self.assertEqual(full.size_done, 1.0)
        self.assertTrue(full.reduce_only)
        self.assertTrue(full.is_maker)

    async def test_replacement_and_empty_snapshot_clear_stale_state(self):
        inventory = Inventory()
        inventory.fees = 12.0
        inventory.funding = 3.0
        inventory.realized = 5.0
        inventory.add_cash(100.0)
        cash = inventory.position_for("USDC")
        client = BulkWebSocketClient(inventory=inventory)
        orders = client.open_orders
        leverage = client.leverage_settings
        await client._handle_account_snapshot({
            "positions": [{"symbol": "BTC-USD", "size": 2.0, "price": 100.0, "liquidationPrice": 40.0},
                          {"symbol": "USDE-USD", "size": -1.0, "price": 50.0}],
            "openOrders": [FULL_ORDER, {**FULL_ORDER, "orderId": "sell-2"}],
            "leverageSettings": [{"symbol": "BTC-USD", "leverage": 10.0}],
        })
        retained = inventory.position_for("BTC-USD")
        removed = inventory.position_for("USDE-USD")
        self.assertEqual(set(orders), {"sell-1", "sell-2"})
        # A reconnect snapshot is authoritative, including absence.
        await client._handle_account_snapshot({
            "positions": [{"symbol": "BTC-USD", "size": 1.0, "price": 101.0, "liquidationPrice": 30.0}],
            "openOrders": [], "leverageSettings": [],
        })
        self.assertIs(inventory.position_for("BTC-USD"), retained)
        self.assertEqual(retained.quantity, 1.0)
        self.assertEqual(retained.liquidation_price, 30.0)
        self.assertNotIn("USDE-USD", inventory.positions)
        self.assertEqual(removed.quantity, 0.0)
        self.assertFalse(orders)
        self.assertFalse(leverage)
        await client._handle_order_update(SHORT_ORDER)
        self.assertIn("sell-1", orders)
        await client._handle_order_update({**SHORT_ORDER, "status": "cancelled"})
        self.assertFalse(orders)
        await client._handle_account_snapshot({"positions": [], "openOrders": [], "leverageSettings": []})
        self.assertEqual(retained.quantity, 0.0)
        self.assertNotIn("BTC-USD", inventory.positions)
        self.assertIs(inventory.position_for("USDC"), cash)
        self.assertEqual(cash.quantity, 100.0)
        self.assertEqual((inventory.fees, inventory.funding, inventory.realized), (12.0, 3.0, 5.0))
        self.assertIs(client.open_orders, orders)
        self.assertIs(client.leverage_settings, leverage)
