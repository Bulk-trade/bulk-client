import base58

from bulk_api.api import BulkHttpClient, BulkWebSocketClient
from bulk_api.common.signer import SignatureDomain, TransactionSigner


def test_signature_domain_registry_is_stable_and_compact():
    assert SignatureDomain.MAINNET.value == 1
    assert SignatureDomain.TESTNET.value == 2
    assert SignatureDomain.DEVNET.value == 3


def test_transaction_signature_is_bound_to_one_domain():
    signer = TransactionSigner("1111111111111111111111111111111111111111111")
    transaction = {
        "actions": [{"faucet": {"u": signer.public_key}}],
        "nonce": "42",
        "account": signer.public_key,
        "signer": signer.public_key,
    }

    signer.sign_transaction(transaction, SignatureDomain.MAINNET)
    mainnet_signature = transaction["signature"]

    assert signer.verify(transaction, SignatureDomain.MAINNET)
    assert not signer.verify(transaction, SignatureDomain.TESTNET)
    assert not signer.verify(transaction, SignatureDomain.DEVNET)
    assert base58.b58decode(mainnet_signature)
    assert "signatureDomain" not in transaction


def test_faucet_signature_matches_rust_testnet_vector():
    signer = TransactionSigner(
        "4XmiBPzjsmugYJtYFmgh8GWYKQEjt2CtT1MqfZKi8pm4tevpthqRePiACfNoUz4DWtxsxtVYHzBYD8PR7qHC21Kc"
    )
    transaction = {
        "actions": [{
            "faucet": {
                "u": "4WzemKSCJP8u2UmUeJzjuRYeTjrM4yqVTUP57MXwXtAA",
                "amount": 1_000_000_000.0,
            }
        }],
        "nonce": 1_785_657_602_556,
        "account": "4WzemKSCJP8u2UmUeJzjuRYeTjrM4yqVTUP57MXwXtAA",
        "signer": "4WzemKSCJP8u2UmUeJzjuRYeTjrM4yqVTUP57MXwXtAA",
    }

    signer.sign_transaction(transaction, SignatureDomain.TESTNET)

    assert transaction["signature"] == (
        "67jDi1jnoefhfYCXFPG2DuACbnA1ChwaDp9Ucz68KphaCMbvq7dudJ4y3h6rx7G1JV1nQNMeQyYcjLK2viFoNyir"
    )


def test_signer_rejects_a_missing_domain():
    signer = TransactionSigner("1111111111111111111111111111111111111111111")
    transaction = {
        "actions": [{"faucet": {"u": signer.public_key}}],
        "nonce": "42",
        "account": signer.public_key,
        "signer": signer.public_key,
    }

    try:
        signer.sign_transaction(transaction, None)
    except ValueError as error:
        assert str(error) == "explicit SignatureDomain is required"
    else:
        raise AssertionError("missing signature domain must fail")


def test_signed_clients_require_an_explicit_domain():
    signer = TransactionSigner("1111111111111111111111111111111111111111111")

    for constructor in (
        lambda: BulkHttpClient(private_key=signer.private_key),
        lambda: BulkWebSocketClient(signer=signer),
    ):
        try:
            constructor()
        except ValueError as error:
            assert "explicit SignatureDomain is required" in str(error)
        else:
            raise AssertionError("configured signer without a domain must fail")
