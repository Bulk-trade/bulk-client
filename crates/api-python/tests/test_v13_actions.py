"""Independent SDK vectors for the new release; no live network access."""
import importlib.util
import json
from pathlib import Path
import pytest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("v13_wire_helpers", ROOT / "tests/test_wire_serialization.py")
helpers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helpers)
signer = helpers.load_signer()
external = helpers.load_module("v13_external_models", ROOT / "bulk_api/messages/external.py")
VECTORS = json.loads((ROOT.parent / "api-rust/src/transaction/fixtures/sdk-v13-vectors.json").read_text())["vectors"]

@pytest.mark.parametrize("vector", VECTORS, ids=lambda vector: vector["name"])
def test_v13_direct_actions_match_sdk(vector):
    if vector["name"] == "nested-admin":
        with pytest.raises(Exception, match="Unknown tx type"):
            signer.TransactionSigner.serialize_transaction(vector["actions"], 42, helpers.PUBKEY, signer.SignatureDomain.DEVNET)
        return
    actual = signer.TransactionSigner.serialize_transaction(vector["actions"], 42, helpers.PUBKEY, signer.SignatureDomain.DEVNET)
    assert actual.hex() == vector["signable_hex"]


def test_external_models_and_global_order_requests():
    fill = external.ExternalFill(123, helpers.PUBKEY, "EXT", helpers.PUBKEY, "BUY", "FX-BTC-USD-EXT", .25, 100000., 1.5)
    assert fill.to_api()["externalFill"]["side"] == "Buy"
    state = external.ExternalOrderState(124, helpers.PUBKEY, helpers.PUBKEY, "SELL", "FX-SOL-USD-EXT", 150., 149.5, 2., 1., "partiallyFilled")
    assert state.to_api()["externalOrderState"]["reason"] is None
    request = external.OrderRequest.from_api(dict(stamp=123, orderId=helpers.PUBKEY, symbol="FX-BTC-USD-EXT", side="Buy", price=100000., size=.25))
    assert request.symbol == "FX-BTC-USD-EXT"
    assert request.order_id == helpers.PUBKEY


def test_global_subscription_and_typed_event_routing():
    import asyncio
    from unittest.mock import AsyncMock
    from bulk_api.api.bulk_ws import BulkWebSocketClient
    from bulk_api.common.topics import Topic

    async def run():
        client = BulkWebSocketClient()
        client._subscribe = AsyncMock()
        await client.subscribe_order_requests()
        subscriptions = client._subscribe.await_args.args[0]
        assert [item.to_dict() for item in subscriptions] == [{"type": "orderRequests"}]
        client._emit_event = AsyncMock()
        await client._handle_message({"type": "orderRequests", "data": {"requests": [
            {"stamp": 123, "orderId": helpers.PUBKEY, "symbol": "FX-BTC-USD-EXT", "side": "Buy", "price": 100000., "size": .25},
            {"stamp": 124, "orderId": helpers.PUBKEY, "symbol": "FX-SOL-USD-EXT", "side": "Sell", "price": 150., "size": 2.},
        ]}})
        topic, requests = client._emit_event.await_args.args
        assert topic == Topic.ORDER_REQUESTS
        assert [item.symbol for item in requests] == ["FX-BTC-USD-EXT", "FX-SOL-USD-EXT"]
    asyncio.run(run())


def test_external_report_submission_signs_without_order_cache_side_effects():
    import asyncio
    from bulk_api.api.bulk_ws import BulkWebSocketClient, ConnectionState
    from bulk_api.common.signer import TransactionSigner, SignatureDomain

    async def run():
        signer = TransactionSigner(helpers.PUBKEY)
        client = BulkWebSocketClient(signer=signer, signature_domain=SignatureDomain.DEVNET)
        sent = []
        class Socket:
            async def send(self, raw):
                request = json.loads(raw)
                sent.append(request)
                client.pending_requests[request["id"]].set_result([])
        client.ws = Socket()
        client.state = ConnectionState.CONNECTED
        report = external.ExternalFill(123, helpers.PUBKEY, "EXT", helpers.PUBKEY, "BUY", "FX-BTC-USD-EXT", .25, 100000., 1.5)
        assert await client.submit_external_reports([report]) == []
        tx = sent[0]["request"]["payload"]
        assert tx["version"] == 12
        assert tx["account"] == signer.public_key
        assert signer.verify(tx, SignatureDomain.DEVNET)
        assert not client.pending_requests
    asyncio.run(run())
