from typing import Dict, List, Tuple
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
            transaction: Transaction dict containing action, account, nonce

        Returns:
            Base58 encoded signature
        """
        # Extract components
        action = tx.get("action", {})
        account = tx.get("account", self.public_key)
        signer = tx.get("signer", self.public_key)
        
        message = self.serialize_transaction(action, account, signer)
        
        signed = self.signing_key.sign(message)
        
        sig = base58.b58encode(signed.signature).decode()
        
        tx["signature"] = sig
        return tx

    @staticmethod
    def generate_account() -> Tuple[str, str]:
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

        # Get the public key (32 bytes)
        public_key_bytes = bytes(signing_key.verify_key)

        # Encode both to base58
        private_key_b58 = base58.b58encode(private_key_bytes).decode()
        public_key_b58 = base58.b58encode(public_key_bytes).decode()

        return private_key_b58, public_key_b58
        
    @staticmethod
    def serialize_transaction(action: Dict, account: str, signer: str) -> bytes:
        """
        Serialize transaction using bincode format

        Args:
            action: Action object with type and parameters
            account: Base58 encoded account public key
            signer: Base58 encoded signer public key

        Returns:
            Binary serialized transaction
        """
        action_type = action.get("type", "")
        parts = [TransactionSigner.serialize_action(action_type)]

        # get / add nonce
        nonce = action.get("nonce", time.time_ns())
        action["nonce"] = nonce

        match action_type:
            case "order":
                parts.extend(TransactionSigner.serialize_orders(action.get("orders", [])))
            case "faucet":
                parts.extend(TransactionSigner.serialize_faucet(action.get("faucet", [])))
            case "updateUserSettings":
                parts.extend(TransactionSigner.serialize_update_user_settings(action.get("settings", [])))
            case "agentWalletCreation":
                parts.extend(TransactionSigner.serialize_agent_wallet_creation(action.get("agent", [])))
            case "testnetAdmin":
                parts.extend(TransactionSigner.serialize_testnet_admin(action.get("actions", [])))
        
        nonce_bytes = TransactionSigner.write_u64(nonce)
        parts.append(nonce_bytes)
        
        parts.append(TransactionSigner.decode_and_validate_key(account))
        parts.append(TransactionSigner.decode_and_validate_key(signer))
        
        return b''.join(parts)
        
    @staticmethod
    def serialize_orders(orders: List[Dict]) -> List[bytes]:
        """Serialize a order list"""
        parts = [TransactionSigner.write_u64(len(orders))]
        
        for order in orders:
            order_type = next(iter(order))
            order_data = order[order_type]

            parts.append(TransactionSigner.serialize_order_type(order_type))
            
            if order_type == "order":
                parts.extend([
                    TransactionSigner.write_string(order_data['c']),
                    TransactionSigner.write_bool(order_data['b']),
                    TransactionSigner.write_f64(order_data['px']),
                    TransactionSigner.write_f64(order_data['sz']),
                    TransactionSigner.write_bool(order_data['r'])
                ])
                
                t = order_data["t"]
                if "limit" in t:
                    parts.extend([
                        TransactionSigner.write_u32(0),
                        TransactionSigner.serialize_time_in_force(t["limit"]["tif"]),
                    ])
                elif "trigger" in t:
                    parts.extend([])
                    
                if "cloid" in order_data:
                    parts.extend([
                        TransactionSigner.write_bool(True),
                        TransactionSigner.write_string(order_data["cloid"]),
                    ])
                else:
                    parts.extend([
                        TransactionSigner.write_bool(False),
                    ])
            elif order_type == "cancel":
                parts.extend([
                    TransactionSigner.write_string(order_data["c"]),
                    base58.b58decode(order_data["oid"]) 
                ])
            elif order_type == "cancelAll":
                parts.extend(
                    [TransactionSigner.write_u64(len(order_data["c"]))]
                    )
                for s in order_data["c"]:
                    parts.extend([
                        TransactionSigner.write_string(s)
                    ])
        return parts
    
    @staticmethod
    def serialize_update_user_settings(settings: Dict) -> List[bytes]:
        """Serialize a update user settings transaction"""
        leverage_map = settings.get("m", [])
        parts = [TransactionSigner.write_u64(len(leverage_map))]
        
        for symbol, leverage in leverage_map:
            parts.extend([
                TransactionSigner.write_string(symbol),
                TransactionSigner.write_f64(leverage)
            ])
            
        return parts

    @staticmethod
    def serialize_faucet(faucet: Dict) -> List[bytes]:
        """Serialize a faucet request"""
        parts = [TransactionSigner.decode_and_validate_key(faucet["u"])]
        
        amount = faucet.get("amount")
        if amount is None:
            parts.append(TransactionSigner.write_bool(False))
        else:
            parts.extend([
                TransactionSigner.write_bool(True),
                TransactionSigner.write_f64(amount)
            ])
            
        return parts
        
    @staticmethod
    def serialize_agent_wallet_creation(agent: Dict) -> List[bytes]:
        """Serialize a agent wallet creation transaction"""
        
        parts = [TransactionSigner.decode_and_validate_key(agent["a"])]
        
        if agent.get("d", False):
            parts.append(TransactionSigner.write_bool(True))
        else:
            parts.append(TransactionSigner.write_bool(False))
        return parts

    @staticmethod
    def serialize_testnet_admin(actions: Dict) -> List[bytes]:
        """Serialize a testnet admin transaction"""
        parts = [TransactionSigner.write_u64(len(actions))]
        for action in actions:
            admin_action = next(iter(action))
            parts.extend([
                TransactionSigner.serialize_admin_actin(admin_action),
                TransactionSigner.decode_and_validate_key(action[admin_action]["account"]),
                TransactionSigner.write_bool(action[admin_action]["whitelist"])
            ])
            
        return parts

    @staticmethod
    def write_u64(value: int) -> bytes:
        """Write a u64 in little-endian format"""
        return struct.pack("<Q", value)

    @staticmethod
    def write_string(value: str) -> bytes:
        """Write a string in little-endian format"""
        s_bytes = value.encode('utf-8')
        return TransactionSigner.write_u64(len(s_bytes)) + s_bytes

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
    def serialize_time_in_force(time_in_force: str) -> bytes:
        if time_in_force not in TIME_IN_FORCE_MAP:
            raise ValueError(f"Invalid time in force: {time_in_force}")
        return struct.pack("<I", TIME_IN_FORCE_MAP[time_in_force])
    
    @staticmethod
    def serialize_admin_actin(admin_action: str) -> bytes:
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