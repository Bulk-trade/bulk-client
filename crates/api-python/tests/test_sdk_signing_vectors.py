import json
from pathlib import Path
import pytest
from bulk_api.common.signer import TransactionSigner, SignatureDomain

FIXTURES = json.loads((Path(__file__).resolve().parents[2] /
    "api-rust/src/transaction/fixtures/sdk-signing-vectors.json").read_text())

@pytest.mark.parametrize("vector", FIXTURES["vectors"], ids=lambda vector: vector["name"])
def test_signing_matches_actual_sdk(vector):
    # Python's current action API does not implement multisig proposal signing.
    if any("msp" in action for action in vector["actions"]):
        with pytest.raises(Exception, match="Unknown tx type"):
            TransactionSigner.serialize_transaction(vector["actions"], vector["nonce"],
                vector["account"], SignatureDomain.DEVNET)
        return
    assert TransactionSigner.serialize_transaction(vector["actions"], vector["nonce"],
        vector["account"], SignatureDomain.DEVNET).hex() == vector["signable_hex"]
