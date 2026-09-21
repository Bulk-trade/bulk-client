"""Live integration test for ALO_SLIDE limit orders.

The test creates a fresh account, funds it from the faucet, receives the BBO
from the WebSocket L2 feed, and submits a crossing buy order.  ALO_SLIDE must
reprice that order below the opposing BBO instead of filling or rejecting it.

Run against a local or staging standalone with:

    BULK_RUN_LIVE_TESTS=1 \
    BULK_HTTP_URL=http://localhost:12000/api/v1 \
    BULK_WS_URL=ws://localhost:12001/ \
    BULK_SIGNATURE_DOMAIN=DEVNET \
    PYTHONPATH=crates/api-python \
    python crates/api-python/tests/test_alo_slide.py
"""

import asyncio
import os
import unittest
from dataclasses import dataclass

from bulk_api.api import BulkHttpClient, BulkWebSocketClient
from bulk_api.common import Side, SignatureDomain, TimeInForce, TransactionSigner


SYMBOL = os.getenv("BULK_ALO_SLIDE_SYMBOL", "BTC-USD")
ORDER_SIZE = float(os.getenv("BULK_ALO_SLIDE_SIZE", "0.001"))
HTTP_URL = os.getenv("BULK_HTTP_URL", "http://localhost:12000/api/v1")
WS_URL = os.getenv("BULK_WS_URL", "ws://localhost:12001/")
TIMEOUT_SECONDS = float(os.getenv("BULK_TEST_TIMEOUT", "15"))


@dataclass(frozen=True)
class Bbo:
    bid: float
    ask: float


def signature_domain() -> SignatureDomain:
    """Return the signature domain selected for the live test."""
    name = os.getenv("BULK_SIGNATURE_DOMAIN", "DEVNET").upper()
    try:
        return SignatureDomain[name]
    except KeyError as error:
        expected = ", ".join(domain.name for domain in SignatureDomain)
        raise ValueError(
            f"invalid BULK_SIGNATURE_DOMAIN={name!r}; expected one of {expected}"
        ) from error


def assert_faucet_deposit(response: dict) -> None:
    """Assert that a faucet response contains a successful deposit status."""
    statuses = response.get("response", {}).get("data", {}).get("statuses", [])
    assert any("deposit" in status for status in statuses), (
        f"faucet response did not contain a deposit: {response}"
    )


async def wait_for_bbo(client: BulkWebSocketClient) -> Bbo:
    """Wait until the WebSocket-maintained book has a two-sided BBO."""
    deadline = asyncio.get_running_loop().time() + TIMEOUT_SECONDS
    while asyncio.get_running_loop().time() < deadline:
        book = client.get_book(SYMBOL)
        if book is not None:
            bid = book.get_best_bid()
            ask = book.get_best_ask()
            if bid is not None and ask is not None:
                return Bbo(bid=bid.price, ask=ask.price)
        await asyncio.sleep(0.05)
    raise TimeoutError(f"did not receive a two-sided {SYMBOL} BBO over WebSocket")


async def wait_for_order(client: BulkWebSocketClient, order_id: str):
    """Wait until the submitted order appears in WebSocket account state."""
    deadline = asyncio.get_running_loop().time() + TIMEOUT_SECONDS
    while asyncio.get_running_loop().time() < deadline:
        order = client.get_order_map().get(order_id)
        if order is not None:
            return order
        await asyncio.sleep(0.05)
    raise TimeoutError(f"order {order_id} did not enter WebSocket account state")


async def run_alo_slide_test() -> None:
    """Fund an account and verify a crossing ALO_SLIDE buy rests below BBO."""
    signer = TransactionSigner.generate_account()
    domain = signature_domain()
    http = BulkHttpClient(
        base_url=HTTP_URL,
        private_key=signer.private_key,
        signature_domain=domain,
    )

    faucet_response = await asyncio.to_thread(http.request_faucet)
    assert_faucet_deposit(faucet_response)

    client = BulkWebSocketClient(
        url=WS_URL,
        symbols=[SYMBOL],
        signer=signer,
        signature_domain=domain,
    )
    order_id = None

    try:
        assert await client.connect(), f"failed to connect to {WS_URL}"
        await client.subscribe_orderbook_snapshot(SYMBOL, nlevels=20)

        initial_bbo = await wait_for_bbo(client)
        aggressive_price = round(initial_bbo.ask * 1.10, 8)
        assert aggressive_price > initial_bbo.ask

        response = await client.place_limit_order(
            SYMBOL,
            Side.BUY,
            aggressive_price,
            ORDER_SIZE,
            time_in_force=TimeInForce.ALO_SLIDE,
        )
        assert not response.is_error(), f"ALO_SLIDE was rejected: {response}"
        assert response.order_id, f"ALO_SLIDE response had no order ID: {response}"
        order_id = response.order_id

        order = await wait_for_order(client, order_id)
        observed_bbo = await wait_for_bbo(client)
        offset = observed_bbo.ask - order.price

        assert order.side == Side.BUY
        assert order.price < aggressive_price, (
            f"order did not slide: submitted={aggressive_price}, resting={order.price}"
        )
        assert offset > 0.0, (
            f"order crossed the live ask: resting={order.price}, ask={observed_bbo.ask}"
        )

        print(
            "ALO_SLIDE observed: "
            f"initial_bbo={initial_bbo.bid}/{initial_bbo.ask}, "
            f"submitted={aggressive_price}, resting={order.price}, "
            f"live_ask={observed_bbo.ask}, offset={offset:.8f}"
        )
    finally:
        if order_id is not None and client.is_connected:
            await client.cancel_order(Side.BUY, SYMBOL, order_id)
        if client.is_connected:
            await client.disconnect()


def test_alo_slide_rests_below_bbo() -> None:
    """Run the live test only when external-state mutation is enabled."""
    if os.getenv("BULK_RUN_LIVE_TESTS") != "1":
        raise unittest.SkipTest("set BULK_RUN_LIVE_TESTS=1 to run live exchange tests")
    asyncio.run(run_alo_slide_test())


if __name__ == "__main__":
    asyncio.run(run_alo_slide_test())
