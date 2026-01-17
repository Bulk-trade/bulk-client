"""
Transaction Signer using bulk-keychain 
"""

import time
from typing import Dict, List, Optional, Tuple

from bulk_keychain import Keypair, Signer

from bulk.common.enums import Side, TimeInForce


class TransactionSigner:
    """
    Example:
        signer = TransactionSigner(private_key_b58)
        print(f"Account: {signer.public_key}")

        # Sign transaction (compatible with existing code)
        tx = signer.sign_transaction(tx)

        # Compute order ID for a limit order
        order_id, nonce = signer.compute_limit_order_id(
            symbol="BTC-USD",
            side=Side.BUY,
            price=99000.0,
            size=0.25,
        )
    """

    __slots__ = ('_keypair', '_signer', '_public_key', '_private_key')

    def __init__(self, private_key: str):
        """
        Initialize signer with base58 encoded private key

        Args:
            private_key: Base58 encoded Ed25519 private key
        """
        self._keypair = Keypair.from_base58(private_key)
        self._signer = Signer(self._keypair)
        self._public_key: str = self._keypair.public_key()
        self._private_key: str = private_key

    @property
    def public_key(self) -> str:
        """Base58 encoded public key"""
        return self._public_key

    @property
    def private_key(self) -> str:
        """Base58 encoded private key"""
        return self._private_key

    # ==================== GENERATION ====================

    @staticmethod
    def generate_account() -> 'TransactionSigner':
        """
        Generate a new random Ed25519 keypair

        Returns:
            TransactionSigner with new keypair
        """
        keypair = Keypair()
        return TransactionSigner(keypair.to_base58())

    # ==================== TRANSACTION SIGNING ====================

    def sign_transaction(self, tx: Dict) -> Dict:
        """
        Sign a transaction with Ed25519

        Args:
            tx: Transaction dict containing action, account, nonce

        Returns:
            Transaction dict with signature added
        """
        action = tx.get("action", {})
        action_type = action.get("type", "")
        nonce = action.get("nonce") or _generate_nonce()
        action["nonce"] = nonce

        # Convert to bulk-keychain format based on action type
        if action_type == "order":
            orders = action.get("orders", [])
            if orders:
                kc_orders = _convert_orders_to_keychain(orders)
                signed = self._signer.sign_group(kc_orders, nonce=nonce)
                tx["signature"] = signed.get("signature") or signed.get("sig", "")
            else:
                tx["signature"] = ""

        elif action_type == "faucet":
            faucet = action.get("faucet", {})
            kc_action = {
                "type": "faucet",
                "user": faucet.get("u"),
                "amount": faucet.get("amount"),
                "nonce": nonce,
            }
            signed = self._signer.sign(kc_action)
            tx["signature"] = signed.get("signature") or signed.get("sig", "")

        elif action_type == "updateUserSettings":
            settings = action.get("settings", {})
            leverage_map = settings.get("m", [])
            kc_action = {
                "type": "updateUserSettings",
                "leverage": [{"symbol": s, "leverage": l} for s, l in leverage_map],
                "nonce": nonce,
            }
            signed = self._signer.sign(kc_action)
            tx["signature"] = signed.get("signature") or signed.get("sig", "")

        elif action_type == "agentWalletCreation":
            agent = action.get("agent", {})
            kc_action = {
                "type": "agentWalletCreation",
                "agent": agent.get("a"),
                "delete": agent.get("d", False),
                "nonce": nonce,
            }
            signed = self._signer.sign(kc_action)
            tx["signature"] = signed.get("signature") or signed.get("sig", "")

        else:
            # Generic signing for other action types
            signed = self._signer.sign(action)
            tx["signature"] = signed.get("signature") or signed.get("sig", "")

        return tx

    # ==================== ORDER ID COMPUTATION ====================

    def compute_limit_order_id(
        self,
        symbol: str,
        side: Side,
        price: float,
        size: float,
        reduce_only: bool = False,
        time_in_force: TimeInForce = TimeInForce.GTC,
        nonce: Optional[int] = None,
    ) -> Tuple[str, int]:
        """
        Compute order ID for a limit order

        Order ID is computed using the same algorithm as the exchange,
        allowing you to know the order ID before submission.

        Args:
            symbol: Trading symbol (e.g. "BTC-USD")
            side: Side.BUY or Side.SELL
            price: Limit price
            size: Order size
            reduce_only: Reduce-only flag
            time_in_force: GTC, IOC, or ALO
            nonce: Optional nonce (auto-generated if not provided)

        Returns:
            Tuple of (order_id, nonce)
        """
        nonce = nonce if nonce is not None else _generate_nonce()

        order_dict = {
            "type": "order",
            "symbol": symbol,
            "is_buy": side == Side.BUY,
            "price": price,
            "size": size,
            "reduce_only": reduce_only,
            "order_type": {"type": "limit", "tif": str(time_in_force)},
            "nonce": nonce,
        }

        signed = self._signer.sign(order_dict)
        order_id = signed.get("order_id") or signed.get("orderId", "")
        return order_id, nonce

    def compute_market_order_id(
        self,
        symbol: str,
        side: Side,
        size: float,
        reduce_only: bool = False,
        nonce: Optional[int] = None,
    ) -> Tuple[str, int]:
        """
        Compute order ID for a market order

        Args:
            symbol: Trading symbol
            side: Side.BUY or Side.SELL
            size: Order size
            reduce_only: Reduce-only flag
            nonce: Optional nonce

        Returns:
            Tuple of (order_id, nonce)
        """
        nonce = nonce if nonce is not None else _generate_nonce()

        order_dict = {
            "type": "order",
            "symbol": symbol,
            "is_buy": side == Side.BUY,
            "price": 0.0,
            "size": size,
            "reduce_only": reduce_only,
            "order_type": {"type": "market", "is_market": True, "trigger_px": 0.0},
            "nonce": nonce,
        }

        signed = self._signer.sign(order_dict)
        order_id = signed.get("order_id") or signed.get("orderId", "")
        return order_id, nonce

    # ==================== BATCH OPERATIONS ====================

    def sign_limit_orders(
        self,
        orders: List[Dict],
        nonce: Optional[int] = None,
    ) -> List[Tuple[str, int]]:
        """
        Sign multiple limit orders and get pre-computed order IDs

        Uses Rust parallel signing for high performance.
        Args:
            orders: List of order dicts with keys:
                - symbol: str
                - side: Side
                - price: float
                - size: float
                - reduce_only: bool (optional)
                - time_in_force: TimeInForce (optional)
            nonce: Base nonce (each order gets nonce + index)

        Returns:
            List of (order_id, nonce) tuples
        """
        base_nonce = nonce if nonce is not None else _generate_nonce()
        order_dicts = []

        for i, order in enumerate(orders):
            order_nonce = base_nonce + i
            tif = order.get('time_in_force', TimeInForce.GTC)

            order_dict = {
                "type": "order",
                "symbol": order['symbol'],
                "is_buy": order['side'] == Side.BUY,
                "price": order['price'],
                "size": order['size'],
                "reduce_only": order.get('reduce_only', False),
                "order_type": {"type": "limit", "tif": str(tif)},
                "nonce": order_nonce,
            }
            order_dicts.append(order_dict)

        # Parallel signing in Rust
        signed_list = self._signer.sign_all(order_dicts)

        results = []
        for i, signed in enumerate(signed_list):
            order_id = signed.get("order_id") or signed.get("orderId", "")
            results.append((order_id, base_nonce + i))

        return results


# ==================== MODULE-LEVEL HELPERS ====================

def _generate_nonce() -> int:
    """Generate nonce from current timestamp (microseconds)"""
    return int(time.time_ns() / 1000)


def _convert_orders_to_keychain(orders: List[Dict]) -> List[Dict]:
    """Convert internal order format to bulk-keychain format"""
    kc_orders = []

    for order_wrapper in orders:
        if "order" in order_wrapper:
            order_data = order_wrapper["order"]
            is_market = "trigger" in order_data.get("t", {})

            if is_market:
                kc_order = {
                    "type": "order",
                    "symbol": order_data['c'],
                    "is_buy": order_data['b'],
                    "price": 0.0,
                    "size": order_data['sz'],
                    "reduce_only": order_data.get('r', False),
                    "order_type": {"type": "market", "is_market": True, "trigger_px": 0.0},
                }
            else:
                tif = order_data.get("t", {}).get("limit", {}).get("tif", "GTC")
                kc_order = {
                    "type": "order",
                    "symbol": order_data['c'],
                    "is_buy": order_data['b'],
                    "price": order_data['px'],
                    "size": order_data['sz'],
                    "reduce_only": order_data.get('r', False),
                    "order_type": {"type": "limit", "tif": tif},
                }
            kc_orders.append(kc_order)

        elif "cancel" in order_wrapper:
            cancel_data = order_wrapper["cancel"]
            kc_order = {
                "type": "cancel",
                "symbol": cancel_data['c'],
                "order_id": cancel_data['oid'],
            }
            kc_orders.append(kc_order)

        elif "cancelAll" in order_wrapper:
            cancel_all_data = order_wrapper["cancelAll"]
            kc_order = {
                "type": "cancelAll",
                "symbols": cancel_all_data.get('c', []),
            }
            kc_orders.append(kc_order)

    return kc_orders
