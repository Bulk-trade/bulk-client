import json
import struct
from typing import Dict, Tuple, List

from nacl.signing import SigningKey
import base58


class TransactionSigner:
    """Handle Ed25519 transaction signing"""

    def __init__(self, private_key_b58: str):
        """
        Initialize signer with base58 encoded private key

        Args:
            private_key_b58: Base58 encoded private key
        """
        # Decode the private key
        private_key_bytes = base58.b58decode(private_key_b58)
        self.signing_key = SigningKey(private_key_bytes[:32])
        self.public_key = base58.b58encode(bytes(self.signing_key.verify_key)).decode()
        self.nonce = 0

    def get_next_nonce(self) -> int:
        """Get the next nonce value"""
        self.nonce += 1
        return self.nonce

    def sign_transaction(self, tx: Dict) -> str:
        """
        Sign a transaction with Ed25519

        Args:
            transaction: Transaction dict containing action, account, nonce

        Returns:
            Base58 encoded signature
        """
        # Extract components
        action = tx.get("action", {})
        account = tx.get("account", self.public_key)
        signer = tx.get("signer", self.public_key)

        # Serialize transaction to binary format
        message = self._serialize_transaction(action, account, signer)

        # Sign the message
        signed = self.signing_key.sign(message)

        # Return base58 encoded signature (just the signature part)
        sig = base58.b58encode(signed.signature).decode()

        # reload sorted tx and add signature
        tx["signature"] = sig
        return tx

    @staticmethod
    def generate_account() -> Tuple[str, str, List[int]]:
        """
        Generate a new random Ed25519 keypair compatible with Solana

        Returns:
            tuple: (private_key_b58, public_key_b58, keypair_dict)
        """
        # Generate a new random signing key
        signing_key = SigningKey.generate()

        # Get the private key (32 bytes seed)
        private_key_bytes = bytes(signing_key)

        # Get the public key (32 bytes)
        public_key_bytes = bytes(signing_key.verify_key)

        # Encode both to base58
        private_key_b58 = base58.b58encode(private_key_bytes).decode()
        public_key_b58 = base58.b58encode(public_key_bytes).decode()

        # Create Solana-compatible keypair format (64 bytes: private + public)
        keypair_bytes = private_key_bytes + public_key_bytes
        keypair_array = list(keypair_bytes)

        return private_key_b58, public_key_b58, keypair_array

    @staticmethod
    def _write_u64(value: int) -> bytes:
        """Write u64 in little-endian format"""
        return struct.pack('<Q', value)

    @staticmethod
    def _write_u32(value: int) -> bytes:
        """Write u32 in little-endian format"""
        return struct.pack('<I', value)

    @staticmethod
    def _write_f64(value: float) -> bytes:
        """Write f64 (double) in little-endian format"""
        return struct.pack('<d', value)

    @staticmethod
    def _write_string(s: str) -> bytes:
        """Write string with length prefix (u64 LE + UTF-8 bytes)"""
        s_bytes = s.encode('utf-8')
        return TransactionSigner._write_u64(len(s_bytes)) + s_bytes

    @staticmethod
    def _write_bool(value: bool) -> bytes:
        """Write boolean as single byte"""
        return bytes([1 if value else 0])

    @staticmethod
    def _serialize_transaction(action: Dict, account: str, signer: str) -> bytes:
        """
        Serialize transaction using bincode format

        Args:
            action: Action object with type and parameters
            account: Base58 encoded account public key
            signer: Base58 encoded signer public key

        Returns:
            Binary serialized transaction
        """
        parts = []
        action_type = action.get("type", "")

        # 1. Transaction type (length-prefixed string)
        parts.append(TransactionSigner._write_string(action_type))

        # 2. Serialize based on action type
        if action_type == "order":
            parts.extend(TransactionSigner._serialize_orders(action.get("orders", [])))

        elif action_type == "cancel":
            parts.extend(TransactionSigner._serialize_cancels(action.get("cancels", [])))

        elif action_type == "cancelall":
            parts.extend(TransactionSigner._serialize_cancelall(action.get("cancels", [])))

        elif action_type == "updateusersettings":
            parts.extend(TransactionSigner._serialize_user_settings(action.get("settings", {})))

        elif action_type == "faucet":
            parts.extend(TransactionSigner._serialize_faucet(action.get("faucet", {})))

        # 3. Account (32 bytes from base58)
        account_bytes = base58.b58decode(account)
        if len(account_bytes) != 32:
            raise ValueError(f"Account must be 32 bytes, got {len(account_bytes)}")
        parts.append(account_bytes)

        # 4. Signer (32 bytes from base58)
        signer_bytes = base58.b58decode(signer)
        if len(signer_bytes) != 32:
            raise ValueError(f"Signer must be 32 bytes, got {len(signer_bytes)}")
        parts.append(signer_bytes)

        # Concatenate all parts
        return b''.join(parts)

    @staticmethod
    def _serialize_orders(orders: List[Dict]) -> List[bytes]:
        """Serialize order list"""
        parts = []

        # Count of orders
        parts.append(TransactionSigner._write_u64(len(orders)))

        # Serialize each order
        for order_wrapper in orders:
            # Unwrap the order if wrapped in 'order' key
            if 'order' in order_wrapper:
                order = order_wrapper['order']
            else:
                order = order_wrapper

            order_parts = []

            # Asset (symbol) - length-prefixed string
            order_parts.append(TransactionSigner._write_string(order['c']))

            # is_buy - boolean
            order_parts.append(TransactionSigner._write_bool(order['b']))

            # price - f64
            order_parts.append(TransactionSigner._write_f64(order['px']))

            # size - f64
            order_parts.append(TransactionSigner._write_f64(order['sz']))

            # reduce_only - boolean
            order_parts.append(TransactionSigner._write_bool(order['r']))

            # order_type - enum with variants
            order_type = order['t']

            if 'limit' in order_type:
                # Limit order (discriminant = 0)
                order_parts.append(TransactionSigner._write_u32(0))

                # TIF: GTC=0, IOC=1, ALO=2
                tif_map = {'GTC': 0, 'IOC': 1, 'ALO': 2}
                tif_str = order_type['limit']['tif']
                tif_value = tif_map.get(tif_str, 0)
                order_parts.append(TransactionSigner._write_u32(tif_value))

            elif 'trigger' in order_type:
                # Trigger order (discriminant = 1)
                order_parts.append(TransactionSigner._write_u32(1))

                # is_market - boolean
                is_market = order_type['trigger'].get('is_market', False)
                order_parts.append(TransactionSigner._write_bool(is_market))

                # triggerPx - f64
                trigger_px = order_type['trigger'].get('triggerPx', 0.0)
                order_parts.append(TransactionSigner._write_f64(trigger_px))

            parts.append(b''.join(order_parts))

        return parts

    @staticmethod
    def _serialize_cancels(cancels: List[Dict]) -> List[bytes]:
        """Serialize cancel order list"""
        parts = []

        # Count of cancels
        parts.append(TransactionSigner._write_u64(len(cancels)))

        # Serialize each cancel
        for cancel in cancels:
            cancel_parts = []

            # Asset (symbol) - length-prefixed string
            cancel_parts.append(TransactionSigner._write_string(cancel['c']))

            # Order ID - length-prefixed string
            cancel_parts.append(TransactionSigner._write_string(cancel['oid']))

            parts.append(b''.join(cancel_parts))

        return parts

    @staticmethod
    def _serialize_cancelall(cancels: List[Dict]) -> List[bytes]:
        """Serialize cancel all list (symbol only, no oid)"""
        parts = []

        # Count of cancels
        parts.append(TransactionSigner._write_u64(len(cancels)))

        # Serialize each cancel (only symbol)
        for cancel in cancels:
            # Asset (symbol) - length-prefixed string
            parts.append(TransactionSigner._write_string(cancel['c']))

        return parts

    @staticmethod
    def _serialize_user_settings(settings: Dict) -> List[bytes]:
        """Serialize user settings (leverage settings)"""
        parts = []

        # Get leverage map
        leverage_map = settings.get('m', [])

        # Count of leverage settings
        parts.append(TransactionSigner._write_u64(len(leverage_map)))

        # Serialize each leverage setting
        for symbol, leverage in leverage_map:
            leverage_parts = []

            # Symbol - length-prefixed string
            leverage_parts.append(TransactionSigner._write_string(symbol))

            # Leverage - f64
            leverage_parts.append(TransactionSigner._write_f64(leverage))

            parts.append(b''.join(leverage_parts))

        return parts

    @staticmethod
    def _serialize_faucet(faucet: Dict) -> List[bytes]:
        """Serialize faucet request"""
        parts = []

        # User address - length-prefixed string
        user = faucet.get('u', '')
        parts.append(TransactionSigner._write_string(user))

        return parts

