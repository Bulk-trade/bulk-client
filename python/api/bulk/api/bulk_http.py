"""
Bulk Labs HTTP REST API Client

Provides complete HTTP REST API access to the Bulk Labs exchange:
- Market data endpoints (unsigned)
- Account query endpoints (unsigned)
- Trading endpoints (signed)
- Private endpoints (signed - faucet, etc.)
"""

import json
import time

import requests
from typing import Dict, List, Optional, Union, Any, Literal

from bulk.common import TransactionSigner, Side, TimeInForce
from bulk.messages import LimitOrder, CancelOrder, MarketOrder, CancelAll


class BulkHttpClient:
    """HTTP REST API client for Bulk Labs exchange"""

    def __init__(
        self,
        base_url: str = "https://exchange-api2.bulk.trade/api/v1",
        private_key: Optional[str] = None,
        timeout: int = 10
    ):
        """
        Initialize HTTP client

        Args:
            base_url: Base URL for API endpoints
            private_key: Base58 encoded private key for signing transactions
            timeout: Request timeout in seconds
        """
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.signer = TransactionSigner(private_key) if private_key else None

    # ===================================================================
    # MARKET DATA ENDPOINTS (PUBLIC, UNSIGNED)
    # ===================================================================

    def get_exchange_info(self) -> Dict:
        """
        Get exchange information including all available markets

        Returns:
            Dict containing:
            - symbols: List of market symbols
            - markets: Detailed market information including tick sizes, lot sizes, etc.

        Example:
            {
                "symbols": ["BTC-USD", "ETH-USD", ...],
                "markets": {
                    "BTC-USD": {
                        "symbol": "BTC-USD",
                        "tickSize": "0.1",
                        "lotSize": "0.0001",
                        ...
                    }
                }
            }
        """
        response = requests.get(
            f"{self.base_url}/exchangeInfo",
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def get_ticker(self, symbol: str) -> Dict:
        """
        Get market ticker/statistics for a symbol

        Args:
            symbol: Market symbol (e.g., "BTC-USD")

        Returns:
            Dict containing:
            - symbol: Market symbol
            - lastPrice: Last traded price
            - volume24h: 24h trading volume
            - priceChange24h: 24h price change
            - priceChangePercent24h: 24h price change percentage
            - highPrice24h: 24h high price
            - lowPrice24h: 24h low price
            - openPrice24h: Price 24h ago
            - markPrice: Current mark price
            - oraclePrice: Current oracle price
            - openInterest: Total open interest
            - fundingRate: Current funding rate

        Example:
            {
                "symbol": "BTC-USD",
                "lastPrice": 100500.0,
                "volume24h": 1234567.89,
                "markPrice": 100450.0,
                "oraclePrice": 100475.0,
                "openInterest": 50000000.0,
                "fundingRate": 0.0001
            }
        """
        response = requests.get(
            f"{self.base_url}/ticker/{symbol}",
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def get_klines(
        self,
        symbol: str,
        interval: Literal["1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w"],
        start_time: Optional[int] = None,
        end_time: Optional[int] = None,
        limit: int = 500
    ) -> List[Dict]:
        """
        Get historical candlestick/OHLCV data

        Args:
            symbol: Market symbol (e.g., "BTC-USD")
            interval: Candle interval (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w)
            start_time: Start timestamp in milliseconds (optional)
            end_time: End timestamp in milliseconds (optional)
            limit: Maximum number of candles to return (default 500, max 1000)

        Returns:
            List of candle dicts containing:
            - t: Open timestamp (milliseconds)
            - T: Close timestamp (milliseconds)
            - o: Open price
            - h: High price
            - l: Low price
            - c: Close price
            - v: Volume
            - n: Number of trades

        Example:
            [
                {
                    "t": 1699564800000,
                    "T": 1699565100000,
                    "o": 100000.0,
                    "h": 100500.0,
                    "l": 99900.0,
                    "c": 100200.0,
                    "v": 123.45,
                    "n": 150
                },
                ...
            ]
        """
        params = {
            "symbol": symbol,
            "interval": interval,
            "limit": limit
        }
        if start_time:
            params["startTime"] = start_time
        if end_time:
            params["endTime"] = end_time

        response = requests.get(
            f"{self.base_url}/klines",
            params=params,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def get_orderbook(
        self,
        symbol: str,
        nlevels: int = 20,
        aggregation: Optional[float] = None
    ) -> Dict:
        """
        Get L2 order book snapshot

        Args:
            symbol: Market symbol (e.g., "BTC-USD")
            nlevels: Number of price levels per side (default 20, max 1000)
            aggregation: Price aggregation/grouping (optional)

        Returns:
            Dict containing:
            - symbol: Market symbol
            - timestamp: Snapshot timestamp
            - bids: List of [price, size, num_orders] arrays
            - asks: List of [price, size, num_orders] arrays

        Example:
            {
                "symbol": "BTC-USD",
                "timestamp": 1699564800000,
                "bids": [
                    [100000.0, 1.5, 3],  # [price, size, num_orders]
                    [99900.0, 2.0, 5],
                    ...
                ],
                "asks": [
                    [100100.0, 1.2, 2],
                    [100200.0, 1.8, 4],
                    ...
                ]
            }
        """
        params = {
            "type": "l2Book",
            "coin": symbol,
            "nlevels": nlevels
        }
        if aggregation is not None:
            params["aggregation"] = aggregation

        response = requests.get(
            f"{self.base_url}/l2book",
            params=params,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    # ===================================================================
    # ACCOUNT ENDPOINTS (PUBLIC, UNSIGNED)
    # ===================================================================

    def get_full_account(self, user: str) -> Dict:
        """
        Get complete account state including positions, orders, and margin

        Args:
            user: User public key (base58)

        Returns:
            Dict containing:
            - positions: List of open positions
            - openOrders: List of resting orders
            - marginSummary: Margin and collateral info
            - settings: Account settings (leverage limits)

        Example:
            {
                "fullAccount": {
                    "positions": [...],
                    "openOrders": [...],
                    "marginSummary": {...},
                    "settings": {...}
                }
            }
        """
        response = requests.post(
            f"{self.base_url}/account",
            json={
                "type": "fullAccount",
                "user": user
            },
            timeout=self.timeout
        )
        response.raise_for_status()
        result = response.json()
        return result[0] if result else {}

    def get_open_orders(self, user: str) -> List[Dict]:
        """
        Get only resting orders for an account

        Args:
            user: User public key (base58)

        Returns:
            List of open orders

        Example:
            [
                {
                    "openOrder": {
                        "coin": "BTC-USD",
                        "orderId": "base58_hash",
                        "orderType": "limit",
                        "price": 99000.0,
                        "origSz": 1000000,
                        "size": 1000000,
                        "filledSz": 0,
                        "isBuy": true,
                        "reduceOnly": false,
                        "status": "placed",
                        "timestamp": 1699564800000
                    }
                },
                ...
            ]
        """
        response = requests.post(
            f"{self.base_url}/account",
            json={
                "type": "openOrders",
                "user": user
            },
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def get_fills(self, user: str) -> List[Dict]:
        """
        Get trade history (up to 5000 recent fills)

        Args:
            user: User public key (base58)

        Returns:
            List of fills/trades

        Example:
            [
                {
                    "fills": {
                        "maker": "maker_pubkey",
                        "taker": "taker_pubkey",
                        "orderIdMaker": "maker_order_hash",
                        "orderIdTaker": "taker_order_hash",
                        "isBuy": true,
                        "symbol": "BTC-USD",
                        "amount": 0.1,
                        "price": 100000.0,
                        "liquidation": false,
                        "timestamp": 1699564800000
                    }
                },
                ...
            ]
        """
        response = requests.post(
            f"{self.base_url}/account",
            json={
                "type": "fills",
                "user": user
            },
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def get_position_history(self, user: str) -> List[Dict]:
        """
        Get closed position history (up to 5000 positions)

        Args:
            user: User public key (base58)

        Returns:
            List of closed positions with P&L

        Example:
            [
                {
                    "positions": {
                        "owner": "user_pubkey",
                        "symbol": "BTC-USD",
                        "maxQuantity": 0.5,
                        "totalVolume": 1.2,
                        "avgOpenPrice": 100000.0,
                        "avgClosePrice": 102500.0,
                        "realizedPnl": 1250.0,
                        "fees": 12.5,
                        "funding": -5.0,
                        "openTime": 1699564800000000000,
                        "closeTime": 1699651200000000000,
                        "closeReason": "normal"
                    }
                },
                ...
            ]
        """
        response = requests.post(
            f"{self.base_url}/account",
            json={
                "type": "positions",
                "user": user
            },
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    # ===================================================================
    # TRADING ENDPOINTS (SIGNED, STATE-MUTATING)
    # ===================================================================

    def place_orders(
        self,
        txns: List[Union[Dict|LimitOrder|MarketOrder|CancelOrder|CancelAll]]
    ) -> Dict:
        """
        Place multiple order-related tx in a single transaction
        - limit orders: either json/dict spec or LimitOrder object
        - market orders: either json/dict spec or MarketOrder object
        - cancel orders: either json/dict spec or CancelOrder object
        - cancel all orders: either json/dict spec or CancelAll object

        Args:
            txns: tx to place in transaction
        """
        
        if not self.signer:
            raise ValueError("Private key required for trading operations")
        
        order_objects = []
        for tx in txns:
            match tx:
                case LimitOrder():
                    order_objects.append(tx.to_api())
                case MarketOrder():
                    order_objects.append(tx.to_api())
                case CancelOrder():
                    order_objects.append(tx.to_api())
                case CancelAll():
                    order_objects.append(tx.to_api())
                case dict():
                    match tx.get("type"):
                        case "order":
                            order = {
                                "c": tx["symbol"],
                                "b": tx["is_buy"],
                                "px": tx["price"],
                                "sz": tx["size"],
                                "r": tx.get("reduce_only", False)
                            }

                            if tx.get("order_type", "limit") == "market":
                                order["t"] = {
                                    "trigger": {
                                        "is_market": True,
                                        "triggerPx": 0.0
                                    }
                                }
                            else:
                                order["t"] = {
                                    "limit": {
                                        "tif": tx.get("time_in_force", "GTC")
                                    }
                                }
                            order_objects.append({"order": order})

                        case "cancel":
                            cancel = {
                                "c": tx["symbol"],
                                "oid": tx["order_id"]
                            }
                            order_objects.append({"cancel": cancel})

                        case "cancelAll":
                            cancelall = {
                                "c": tx["symbols"]
                            }
                            order_objects.append({"cancelAll": cancelall})
                        case _:
                            raise ValueError(f"Invalid order type: {tx.get('type')}")
                case _:
                    raise ValueError("Invalid txn type: {}".format(tx))

        # package into a transaction
        transaction = {
            "action": {
                "type": "order",
                "orders": order_objects
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }
        
        transaction = self.signer.sign_transaction(transaction)
        ser = json.dumps(transaction)
        
        # Send request
        response = requests.post(
            f"{self.base_url}/order",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def update_leverage(self, leverage_settings: List[tuple]) -> Dict:
        """
        Update maximum leverage settings for markets

        Args:
            leverage_settings: List of (symbol, max_leverage) tuples
                Example: [("BTC-USD", 5.0), ("ETH-USD", 3.0)]

        Returns:
            API response with update status

        Example:
            response = client.update_leverage([
                ("BTC-USD", 5.0),
                ("ETH-USD", 3.0)
            ])
        """
        if not self.signer:
            raise ValueError("Private key required for settings operations")

        # Build transaction
        transaction = {
            "action": {
                "type": "updateUserSettings",
                "settings": {
                    "m": leverage_settings
                },
                "nonce": 1
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        transaction = self.signer.sign_transaction(transaction)

        # Send request
        response = requests.post(
            f"{self.base_url}/user-settings",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def manage_agent_wallet(self, agent_pubkey: str, delete: bool = False) -> Dict:
        """
        Create or delete an agent wallet authorization

        Args:
            agent_pubkey: Agent's public key (base58)
            delete: True to delete agent, False to create/add

        Returns:
            API response with operation status

        Example:
            # Add agent
            response = client.manage_agent_wallet(
                agent_pubkey="5Am6JkEHAjYG1itNWRMGpQrxvY8AaqkXCo1TZvenqVux",
                delete=False
            )

            # Remove agent
            response = client.manage_agent_wallet(
                agent_pubkey="5Am6JkEHAjYG1itNWRMGpQrxvY8AaqkXCo1TZvenqVux",
                delete=True
            )
        """
        if not self.signer:
            raise ValueError("Private key required for agent operations")

        # Build transaction
        transaction = {
            "action": {
                "type": "agentWalletCreation",
                "agent": {
                    "a": agent_pubkey,
                    "d": delete
                }
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        transaction = self.signer.sign_transaction(transaction)

        # Send request
        response = requests.post(
            f"{self.base_url}/agent-wallet",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    # ===================================================================
    # PRIVATE ENDPOINTS (SIGNED)
    # ===================================================================

    def whitelist_faucet(
        self,
        target_account: str,
        whitelist: bool = True,
        nonce: Optional[int] = None
    ) -> Dict:
        """
        Whitelist or unwhitelist an account for testnet faucet access

        TESTNET ADMIN ONLY - This endpoint requires admin privileges

        Args:
            target_account: Target user's public key (base58) to whitelist/unwhitelist
            whitelist: True to whitelist, False to unwhitelist
            nonce: Optional nonce (defaults to current time in milliseconds)

        Returns:
            API response with operation status

        Example:
            # Whitelist an account
            response = client.whitelist_faucet(
                target_account="5Am6JkEHAjYG1itNWRMGpQrxvY8AaqkXCo1TZvenqVux",
                whitelist=True
            )

            # Unwhitelist an account
            response = client.whitelist_faucet(
                target_account="5Am6JkEHAjYG1itNWRMGpQrxvY8AaqkXCo1TZvenqVux",
                whitelist=False
            )
        """
        if not self.signer:
            raise ValueError("Private key required for admin operations")

        # Use current time in milliseconds if nonce not provided
        if nonce is None:
            nonce = time.time_ns()

        # Build transaction
        transaction = {
            "action": {
                "type": "testnetAdmin",
                "actions": [
                    {
                        "whitelistFaucet": {
                            "account": target_account,
                            "whitelist": whitelist
                        }
                    }
                ],
                "nonce": nonce
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signed_tx = self.signer.sign_transaction(transaction)

        # Send request
        response = requests.post(
            f"{self.base_url}/private/testnet-admin",
            json=signed_tx,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def request_faucet(self, user: Optional[str] = None, amount = None, nonce: int = time.time_ns()) -> Dict:
        """
        Request testnet faucet funds (100,000 USDC)

        TESTNET ONLY - This endpoint is only available on testnet environments

        Args:
            user: Optional user public key (defaults to signer's public key)
            amount: Optional amount of funds to request (only for whitelisted accounts)
            nonce: Optional nonce (defaults to current time ns)

        Returns:
            API response with success status or error

        Example:
            # Request funds for current account
            response = client.request_faucet()

            # Request funds for specific account
            response = client.request_faucet(user="other_pubkey")
        """
        if not self.signer:
            raise ValueError("Private key required for faucet operations")

        target_user = user or self.signer.public_key

        # Build transaction
        if amount is None:
            transaction = {
                "action": {
                    "type": "faucet",
                    "faucet": {
                        "u": target_user
                    },
                    "nonce": nonce
                },
                "account": target_user,
                "signer": self.signer.public_key
            }
        else:
            transaction = {
                "action": {
                    "type": "faucet",
                    "faucet": {
                        "u": target_user,
                        "amount": amount
                    },
                    "nonce": nonce
                },
                "account": target_user,
                "signer": self.signer.public_key
            }

        # Sign transaction
        tx = self.signer.sign_transaction(transaction)
        
        print(f"Sending tx: {tx}")
        
        # Send request
        response = requests.post(
            f"{self.base_url}/private/faucet",
            json=tx,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()


# ===================================================================
# EXAMPLE USAGE
# ===================================================================

def load_or_create_keys(key_file: str = "/tmp/bulk_keys") -> tuple[str, str]:
    """
    Load existing keys from file or create new ones and save them

    Args:
        key_file: Path to key storage file

    Returns:
        Tuple of (private_key, public_key)
    """
    if os.path.exists(key_file):
        # Load existing keys
        with open(key_file, 'r') as f:
            keys = json.load(f)
            private_key = keys['private_key']
            public_key = keys['public_key']
            print(f"Loaded existing keys from {key_file}")
            print(f"Public key: {public_key}")
            return private_key, public_key
    else:
        # Generate new keys and save them
        private_key, public_key = TransactionSigner.generate_account()
        keys = {
            'private_key': private_key,
            'public_key': public_key
        }

        # Request faucet (testnet only)
        base_url = "https://exchange-api2.bulk.trade/api/v1"
        client = BulkHttpClient(base_url=base_url, private_key=private_key)
        try:
            faucet_result = client.request_faucet()
            print(f"Faucet request: {faucet_result}")
        except Exception as e:
            print(f"Faucet error (expected on mainnet): {e}")

        # Ensure directory exists
        os.makedirs(os.path.dirname(key_file) if os.path.dirname(key_file) else '.', exist_ok=True)

        with open(key_file, 'w') as f:
            json.dump(keys, f, indent=2)
        print(f"Generated new keys and saved to {key_file}")
        print(f"Public key: {public_key}")

        return private_key, public_key

if __name__ == "__main__":
    import os

    private_key, pub_key = load_or_create_keys()

    base_url = "https://exchange-api2.bulk.trade/api/v1"
    client = BulkHttpClient(base_url=base_url, private_key=private_key)


    # Example 1: Get orderbook
    orderbook = client.get_orderbook("BTC-USD", nlevels=5)
    bids = orderbook["levels"][0]
    asks = orderbook["levels"][1]
    print(f"Top bid: ${bids[0]}")
    print(f"Top ask: ${asks[0]}")

    # Example 2: Trading operations (requires private key)
    print("\n=== Trading Operations (Signed) ===")

    # Place a limit order (example - adjust price/size as needed)
    order_result = client.place_orders([LimitOrder(
        symbol="BTC-USD",
        side=Side.BUY,
        price=100000.0,
        size=0.001,
        time_in_force=TimeInForce.GTC,
    )])
    print(f"Order placed: {order_result}")

    # Example 3: Update leverage
    try:
        leverage_result = client.update_leverage([
            ("BTC-USD", 5.0),
            ("ETH-USD", 3.0)
        ])
        print(f"Leverage updated: {leverage_result}")
    except Exception as e:
        print(f"Leverage update: {e}")


    # Example 4: Read-only market data access (no private key needed)
    print("=== Read-only Market Data ===")

    # Get exchange info
    exchange_info = client.get_exchange_info()
    print(f"Excahnge Info: {exchange_info}")

    # Get ticker
    ticker = client.get_ticker("BTC-USD")
    print(f"BTC-USD last price: ${ticker.get('lastPrice', 0):,.2f}")
    # Get candles
    candles = client.get_klines("BTC-USD", "1h", limit=5)
    print(f"Latest candle close: ${candles[-1]['c']:,.2f}")

    # Example 2: Account queries (no private key needed)
    print("\n=== Account Queries (Unsigned) ===")
    test_user = pub_key

    # Get full account
    account = client.get_full_account(test_user)
    if account:
        print(f"Account has {len(account.get('fullAccount', {}).get('positions', []))} positions")

    # Get open orders
    orders = client.get_open_orders(test_user)
    print(f"Found {len(orders)} open orders")
