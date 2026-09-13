import unittest
import base58
from nacl.signing import SigningKey
from bulk_api.common.signer import TransactionSigner, SignatureDomain
from bulk_api.api.bulk_http import BulkHttpClient


class PrivateKeyValidation(unittest.TestCase):
    def test_exact_lengths_without_secret_in_errors(self):
        for length in (0, 31, 33, 63, 65):
            encoded = base58.b58encode(bytes([7]) * length).decode()
            with self.subTest(length=length):
                with self.assertRaises(ValueError) as error:
                    TransactionSigner(encoded)
                if encoded:
                    self.assertNotIn(encoded, str(error.exception))

    def test_seed_and_full_keypair_preserve_identity(self):
        seed = bytes([7]) * 32
        public = bytes(SigningKey(seed).verify_key)
        for raw in (seed, seed + public):
            signer = TransactionSigner(base58.b58encode(raw).decode())
            self.assertEqual(signer.public_key, base58.b58encode(public).decode())

    def test_mismatched_public_suffix_is_rejected(self):
        seed = bytes([7]) * 32
        with self.assertRaises(ValueError):
            TransactionSigner(base58.b58encode(seed + bytes(32)).decode())

    def test_explicit_empty_http_key_is_not_read_only(self):
        with self.assertRaises(ValueError):
            BulkHttpClient(private_key="", signature_domain=SignatureDomain.MAINNET)
        self.assertIsNone(BulkHttpClient(private_key=None).signer)

    def test_http_preserves_supplied_identity(self):
        seed = bytes([7]) * 32
        client = BulkHttpClient(
            private_key=base58.b58encode(seed).decode(),
            signature_domain=SignatureDomain.MAINNET,
        )
        self.assertEqual(client.signer.public_key, base58.b58encode(
            bytes(SigningKey(seed).verify_key)).decode())

    def test_http_demo_uses_explicit_key_without_generating(self):
        import runpy
        import importlib
        from unittest.mock import patch

        encoded = base58.b58encode(bytes([7]) * 32).decode()
        class StopBeforeNetwork(Exception):
            pass

        def inspect_key(signer, private_key):
            self.assertEqual(private_key, encoded)
            raise StopBeforeNetwork

        with patch.dict("os.environ", {"BULK_PRIVATE_KEY": encoded}), \
                patch.object(TransactionSigner, "__init__", inspect_key), \
                patch.object(TransactionSigner, "generate_account", side_effect=AssertionError("unexpected generation")):
            with self.assertRaises(StopBeforeNetwork):
                runpy.run_path(importlib.import_module("bulk_api.api.bulk_http").__file__, run_name="__main__")
