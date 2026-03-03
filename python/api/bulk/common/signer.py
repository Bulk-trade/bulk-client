from typing import Dict, List, Tuple, Optional
from nacl.signing import SigningKey
import struct
import base58
import time

ACTION_CODES = {
    "order": 0,
    "oracle": 1,
    "faucet": 2,
    "updateUserSettings": 3,
    "agentWalletCreation": 4,
    "testnetAdmin": 5,
}

ORDER_MAP = {
    "order": 0,
    "cancel": 1,
    "cancelAll": 2,
}

TIME_IN_FORCE_MAP = {
    "GTC": 0,
    "IOC": 1,
    "ALO": 2,
}

ADMIN_ACTION_MAP = {
    "whitelistFaucet": 0
}

class TransactionSigner:
    """Handle Ed25519 transaction signing with serialization of bulk payloads"""
    
    def __init__(self, private_key: str):
        """
        Initialize signer with base58 encoded private key
        
        Args:
            private_key: Base58 encoded private key
        """
        private_key_bytes = base58.b58decode(private_key)
        self.signing_key = SigningKey(private_key_bytes[:32])
        self.public_key = base58.b58encode(bytes(self.signing_key.verify_key)).decode()
        self.private_key = private_key
        self.nonce = 0
        
    def sign_transaction(self, tx: Dict) -> str:
        """
        Sign a transaction with Ed25519

        Args:
            tx: Transaction dict containing action, account, nonce

        Returns:
            Base58 encoded signature
        """
        # Extract components
        action = tx.get("actions", [])
        nonce = tx.get("nonce", int(time.time_ns() / 1000))
        account = tx.get("account", self.public_key)
        signer = tx.get("signer", self.public_key)
        
        message = self.serialize_transaction(action, nonce, account)
        signed = self.signing_key.sign(message)
        
        sig = base58.b58encode(signed.signature).decode()
        
        tx["signature"] = sig
        return tx

    @staticmethod
    def generate_account() -> 'TransactionSigner':
        """
        Generate a new random Ed25519 keypair compatible with Solana (this is just used for testing
        and is not a recommended way to generate accounts).

        Returns:
            tuple: (private_key_b58, public_key_b58)
        """
        # Generate a new random signing key
        signing_key = SigningKey.generate()

        # Get the private key (32 bytes seed)
        private_key_bytes = bytes(signing_key)

        # Encode both to base58
        private_key_b58 = base58.b58encode(private_key_bytes).decode()
        return TransactionSigner(private_key_b58)
        
    @staticmethod
    def serialize_transaction(
        actions: List[Dict],
        nonce: int,
        account: str) -> bytes:
        """
        Serialize transaction using bincode format

        Args:
            actions: action list
            nonce: nonce
            account: Base58 encoded account public key

        Returns:
            Binary serialized transaction
        """
        parts = [TransactionSigner.write_u32(len(actions))]
        for action in actions:
            parts.append(TransactionSigner.serialize_action(action))

        parts.append(TransactionSigner.write_u64(nonce))
        parts.append(TransactionSigner.decode_and_validate_key(account))
        return b''.join(parts)

    @staticmethod
    def serialize_action(self, action: dict) -> bytes:
        def to_fixedpoint(x: float) -> int:
            return int(round(x * 1e8))

        match action:
            case {"m": order}:
                return b''.join([
                    TransactionSigner.write_u32(0),
                    TransactionSigner.write_string(order['c']),
                    TransactionSigner.write_bool(order['b']),
                    TransactionSigner.write_u64(to_fixedpoint(order['sz'])),
                    TransactionSigner.write_bool(order['r']),
                ])

            case {"l": order}:
                return b''.join([
                    TransactionSigner.write_u32(1),
                    TransactionSigner.write_string(order['c']),
                    TransactionSigner.write_bool(order['b']),
                    TransactionSigner.write_u64(to_fixedpoint(order['px'])),
                    TransactionSigner.write_u64(to_fixedpoint(order['sz'])),
                    TransactionSigner.write_u32(TIME_IN_FORCE_MAP[order["tif"]]),
                    TransactionSigner.write_bool(order['r']),
                ])

            case {"mod": order}:
                return b''.join([
                    TransactionSigner.write_u32(2),
                    TransactionSigner.decode_and_validate_key(order['oid']),
                    TransactionSigner.write_string(order['symbol']),
                ])

            case {"cx": order}:
                return b''.join([
                    TransactionSigner.write_u32(3),
                    TransactionSigner.write_string(order['c']),
                    TransactionSigner.decode_and_validate_key(order['oid']),
                ])

            case {"cxa": order}:
                return b''.join([
                    TransactionSigner.write_u32(4),
                    TransactionSigner.write_strings(order['c']),
                ])

            case {"p": order}:
                return b''.join([
                    TransactionSigner.write_u32(5),
                    TransactionSigner.write_u64(order['t']),
                    TransactionSigner.write_string(order['c']),
                    TransactionSigner.write_f64(order['px']),
                ])

            case {"o": order}:
                oracles = order["oracles"]
                parts = [
                    TransactionSigner.write_u32(6),
                    TransactionSigner.write_u32(len(oracles)),
                ]
                for x in oracles:
                    parts.append(TransactionSigner.write_u64(x['t']))
                    parts.append(TransactionSigner.write_u64(x['fi']))
                    parts.append(TransactionSigner.write_u64(x['px']))
                    parts.append(TransactionSigner.write_i16(x['e']))

                return b''.join(parts)

            case {"faucet": order}:
                if "amount" in order:
                    return b''.join([
                        TransactionSigner.write_u32(7),
                        TransactionSigner.decode_and_validate_key(order['u']),
                        TransactionSigner.write_bool(True),
                        TransactionSigner.write_f64(order['amount']),
                    ])
                else:
                    return b''.join([
                        TransactionSigner.write_u32(7),
                        TransactionSigner.decode_and_validate_key(order['u']),
                        TransactionSigner.write_bool(False),
                    ])

            case {"agentWalletCreation": order}:
                return b''.join([
                    TransactionSigner.write_u32(8),
                    TransactionSigner.decode_and_validate_key(order['a']),
                    TransactionSigner.write_bool(order['d']),
                ])

            case {"updateUserSettings": order}:
                settings = order["m"]
                parts = [
                    TransactionSigner.write_u32(9),
                    TransactionSigner.write_u32(len(settings)),
                ]
                for key,value in settings:
                    parts.append(TransactionSigner.write_string(key))
                    parts.append(TransactionSigner.write_f64(value))

                return b''.join(parts)

            case {"whiteListFaucet": order}:
                return b''.join([
                    TransactionSigner.write_u32(10),
                    TransactionSigner.decode_and_validate_key(order['target']),
                    TransactionSigner.write_bool(order['whitelist']),
                ])
            case _:
                raise Exception("Unknown tx type")
        

    @staticmethod
    def write_u64(value: int) -> bytes:
        """Write a u64 in little-endian format"""
        return struct.pack("<Q", value)

    @staticmethod
    def write_i16(value: int) -> bytes:
        """Write a i16 in little-endian format"""
        return struct.pack("<h", value)

    @staticmethod
    def write_string(value: str) -> bytes:
        """Write a string in little-endian format"""
        s_bytes = value.encode('utf-8')
        return TransactionSigner.write_u64(len(s_bytes)) + s_bytes

    @staticmethod
    def write_strings(value: List[str]) -> bytes:
        """Write a string in little-endian format"""
        bytes = TransactionSigner.write_u32(len(value))
        for x in value:
            bytes.extend(TransactionSigner.write_string(x))
        return bytes

    @staticmethod
    def write_bool(value: bool) -> bytes:
        """Write a boolean as single byte"""
        return bytes([1 if value else 0])

    @staticmethod
    def write_f64(value: float) -> bytes:
        """Write a f64 (double) in little-endian format"""
        return struct.pack("<d", value)

    @staticmethod
    def write_u32(value: int) -> bytes:
        """Write a u32 in little-endian format"""
        return struct.pack("<I", value)

    @staticmethod
    def serialize_order_type(order_type: str) -> bytes:
        if order_type not in ORDER_MAP:
            raise ValueError(f"Invalid order type: {order_type}")
        return struct.pack("<I", ORDER_MAP[order_type])

    @staticmethod
    def serialize_action(action_type: str) -> bytes:
        if action_type not in ACTION_CODES:
            raise ValueError(f"Invalid action type: {action_type}")
        return struct.pack("<I", ACTION_CODES[action_type])
    
    @staticmethod
    def serialize_admin_action(admin_action: str) -> bytes:
        if admin_action not in ADMIN_ACTION_MAP:
            raise ValueError(f"Invalid admin action: {admin_action}")
        return struct.pack("<I", ADMIN_ACTION_MAP[admin_action])
    
    @staticmethod
    def decode_and_validate_key(key: str) -> bytes:
        """Decode a base58 public key"""
        key_bytes = base58.b58decode(key)
        
        if len(key_bytes) != 32:
            raise ValueError(f"Key must be 32 bytes, got {len(key_bytes)}")
        return key_bytes