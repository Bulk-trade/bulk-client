import importlib.util
import struct
import sys
import types
from enum import Enum
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PUBKEY = "11111111111111111111111111111111"
BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
MISSING = object()


def load_module(name: str, path: Path, patches=None):
    patches = patches or {}
    previous = {key: sys.modules.get(key, MISSING) for key in patches}
    sys.modules.update(patches)
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        for key, value in previous.items():
            if value is MISSING:
                sys.modules.pop(key, None)
            else:
                sys.modules[key] = value
    return module


def load_signer():
    exceptions = types.ModuleType("nacl.exceptions")
    exceptions.BadSignatureError = Exception

    signing = types.ModuleType("nacl.signing")
    signing.SigningKey = object
    signing.VerifyKey = object

    nacl = types.ModuleType("nacl")
    nacl.exceptions = exceptions
    nacl.signing = signing

    base58 = types.ModuleType("base58")
    base58.b58decode = b58decode

    return load_module(
        "bulk_test_signer",
        ROOT / "bulk_api/common/signer.py",
        {
            "nacl": nacl,
            "nacl.exceptions": exceptions,
            "nacl.signing": signing,
            "base58": base58,
        },
    )


def b58decode(value: str) -> bytes:
    decoded = 0
    for char in value:
        decoded = decoded * 58 + BASE58_ALPHABET.index(char)
    payload = decoded.to_bytes((decoded.bit_length() + 7) // 8, "big") if decoded else b""
    return b"\0" * (len(value) - len(value.lstrip("1"))) + payload


def load_trade():
    class Side(Enum):
        BUY = 1
        SELL = 0

    class TimeInForce(Enum):
        GTC = "GTC"
        IOC = "IOC"
        ALO = "ALO"

    class OrderStatus(Enum):
        ERROR = None

    common = types.ModuleType("bulk_api.common")
    common.OrderStatus = OrderStatus
    common.Side = Side
    common.TimeInForce = TimeInForce

    messages = types.ModuleType("bulk_api.messages")
    messages.OrderState = object

    package = types.ModuleType("bulk_api")
    package.common = common
    package.messages = messages

    return load_module(
        "bulk_test_trade",
        ROOT / "bulk_api/messages/trade.py",
        {
            "bulk_api": package,
            "bulk_api.common": common,
            "bulk_api.messages": messages,
        },
    )


def test_trailing_stop_uses_signer_field_name():
    trade = load_trade()
    signer = load_signer()

    action = trade.TrailingStop(
        symbol="BTC-USD",
        side=trade.Side.BUY,
        size=0.5,
        trail_bps=800,
        step_bps=100,
        limit=50_000.0,
    ).to_api()

    assert "trb" in action["trl"]
    assert "tdb" not in action["trl"]
    assert signer.TransactionSigner.serialize_action(action).startswith(struct.pack("<I", 9))


def test_whitelist_faucet_accepts_client_payload_shape():
    signer = load_signer()

    encoded = signer.TransactionSigner.serialize_action(
        {"whitelistFaucet": {"account": PUBKEY, "whitelist": True}}
    )

    assert encoded.startswith(struct.pack("<I", 19))
    assert encoded.endswith(bytes([1]))


def test_whitelist_faucet_accepts_legacy_payload_shape():
    signer = load_signer()

    encoded = signer.TransactionSigner.serialize_action(
        {"whiteListFaucet": {"target": PUBKEY, "whitelist": False}}
    )

    assert encoded.startswith(struct.pack("<I", 19))
    assert encoded.endswith(bytes([0]))


def test_conditional_models_emit_canonical_iso_and_inline_on_fill_fields():
    trade = load_trade()

    assert trade.Stop("BTC-USD", True, 1.0, 100.0, iso=True).to_api()["st"]["i"] is True
    assert (
        trade.TakeProfit("BTC-USD", False, 1.0, 100.0, iso=True).to_api()["tp"]["i"]
        is True
    )
    assert (
        trade.Range(
            "BTC-USD",
            True,
            1.0,
            90.0,
            110.0,
            iso=True,
        ).to_api()["rng"]["i"]
        is True
    )
    assert (
        trade.TrailingStop(
            "BTC-USD",
            trade.Side.BUY,
            1.0,
            100,
            10,
            iso=True,
        ).to_api()["trl"]["i"]
        is True
    )

    trigger = trade.Trigger(
        "BTC-USD",
        True,
        100.0,
        [{"m": {"c": "BTC-USD", "b": True, "sz": "1", "r": False, "i": False}}],
    ).to_api()
    assert "i" not in trigger["trig"]

    on_fill = trade.OnFill(
        trigger={"l": {
            "c": "BTC-USD",
            "b": True,
            "px": "100",
            "sz": "1",
            "tif": "GTC",
            "r": False,
            "i": False,
        }},
        actions=[],
    ).to_api()
    assert set(on_fill["of"]) == {"trigger", "actions"}
    assert set(on_fill["of"]["trigger"]) == {"l"}


def test_python_trigger_and_on_fill_signing_match_current_sdk_vectors():
    signer = load_signer()
    builder_code = {"to": PUBKEY, "fee": 5}

    trigger = {
        "trig": {
            "c": "BTC-USD",
            "d": True,
            "tr": 100_000.0,
            "actions": [
                {"m": {
                    "c": "BTC-USD", "b": True, "sz": 1.25,
                    "r": False, "i": True,
                }},
                {"l": {
                    "c": "ETH-USD", "b": False, "px": 2500.5, "sz": 2.0,
                    "tif": "ALO", "r": True, "i": False,
                    "builderCode": builder_code,
                }},
            ],
        }
    }
    trigger_bytes = signer.TransactionSigner.serialize_transaction(
        [trigger], 7, PUBKEY, signer.SignatureDomain.TESTNET
    )
    expected_trigger = "01000000000000000800000007000000000000004254432d5553440100a0724e1809000002000000000000000000000007000000000000004254432d55534401405973070000000000010100000007000000000000004554482d55534400803424383a00000000c2eb0b00000000020000000100010000000000000000000000000000000000000000000000000000000000000000050700000000000000000000000000000000000000000000000000000000000000000000000000000002"
    assert trigger_bytes.hex() == expected_trigger, trigger_bytes.hex()

    on_fill = {
        "of": {
            "trigger": {"l": {
                "c": "ETH-USD", "b": False, "px": 2500.5, "sz": 2.0,
                "tif": "ALO", "r": True, "i": False,
            }},
            "actions": [
                {"m": {
                    "c": "BTC-USD", "b": True, "sz": 1.25,
                    "r": False, "i": True,
                }},
                {"m": {
                    "c": "BTC-USD", "b": True, "sz": 1.25,
                    "r": False, "i": True, "builderCode": builder_code,
                }},
            ],
        }
    }
    on_fill_bytes = signer.TransactionSigner.serialize_transaction(
        [on_fill], 7, PUBKEY, signer.SignatureDomain.TESTNET
    )
    expected_on_fill = "01000000000000000a0000000100000007000000000000004554482d55534400803424383a00000000c2eb0b0000000002000000010002000000000000000000000007000000000000004254432d55534401405973070000000000010000000007000000000000004254432d5553440140597307000000000001010000000000000000000000000000000000000000000000000000000000000000050700000000000000000000000000000000000000000000000000000000000000000000000000000002"
    assert on_fill_bytes.hex() == expected_on_fill, on_fill_bytes.hex()


if __name__ == "__main__":
    test_trailing_stop_uses_signer_field_name()
    test_whitelist_faucet_accepts_client_payload_shape()
    test_whitelist_faucet_accepts_legacy_payload_shape()
    test_conditional_models_emit_canonical_iso_and_inline_on_fill_fields()
    test_python_trigger_and_on_fill_signing_match_current_sdk_vectors()
