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


if __name__ == "__main__":
    test_trailing_stop_uses_signer_field_name()
    test_whitelist_faucet_accepts_client_payload_shape()
    test_whitelist_faucet_accepts_legacy_payload_shape()
