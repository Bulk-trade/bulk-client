"""Independent SDK vectors for the new release; no live network access."""
import importlib.util
import json
from pathlib import Path
import pytest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("v12_wire_helpers", ROOT / "tests/test_wire_serialization.py")
helpers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helpers)
signer = helpers.load_signer()
VECTORS = json.loads((ROOT.parent / "api-rust/src/transaction/fixtures/sdk-v12-vectors.json").read_text())["vectors"]

@pytest.mark.parametrize("vector", VECTORS, ids=lambda vector: vector["name"])
def test_v12_direct_actions_match_sdk(vector):
    if vector["name"] == "nested-admin":
        with pytest.raises(Exception, match="Unknown tx type"):
            signer.TransactionSigner.serialize_transaction(vector["actions"], 42, helpers.PUBKEY, signer.SignatureDomain.DEVNET)
        return
    actual = signer.TransactionSigner.serialize_transaction(vector["actions"], 42, helpers.PUBKEY, signer.SignatureDomain.DEVNET)
    assert actual.hex() == vector["signable_hex"]
