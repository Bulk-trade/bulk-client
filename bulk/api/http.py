"""
Bulk Labs HTTP REST API Client

Provides complete HTTP REST API access to the Bulk Labs exchange:
- Market data endpoints (unsigned)
- Account query endpoints (unsigned)
- Trading endpoints (signed)
- Private endpoints (signed - faucet, etc.)
"""

import json
import requests
from typing import Dict, List, Optional, Union, Any, Literal

from bulk.common import TransactionSigner


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
            "type": "l2Snapshot",
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

    def place_order(
        self,
        symbol: str,
        is_buy: bool,
        price: float,
        size: float,
        order_type: Literal["limit", "market"] = "limit",
        time_in_force: Literal["GTC", "IOC", "ALO"] = "GTC",
        reduce_only: bool = False,
        client_order_id: Optional[str] = None
    ) -> Dict:
        """
        Place a new order

        Args:
            symbol: Market symbol (e.g., "BTC-USD")
            is_buy: True for buy, False for sell
            price: Order price (use 0.0 for market orders)
            size: Order size
            order_type: "limit" or "market"
            time_in_force: "GTC" (Good Till Cancel), "IOC" (Immediate or Cancel), "ALO" (Add Liquidity Only)
            reduce_only: True if order should only reduce position
            client_order_id: Optional client order ID (base58)

        Returns:
            API response with order status

        Example:
            response = client.place_order(
                symbol="BTC-USD",
                is_buy=True,
                price=99000.0,
                size=0.1,
                order_type="limit",
                time_in_force="GTC"
            )
        """
        if not self.signer:
            raise ValueError("Private key required for trading operations")

        # Build order object
        order = {
            "c": symbol,
            "b": is_buy,
            "px": price,
            "sz": size,
            "r": reduce_only
        }

        # Set order type
        if order_type == "market":
            order["t"] = {
                "trigger": {
                    "is_market": True,
                    "triggerPx": 0.0
                }
            }
        else:
            order["t"] = {
                "limit": {
                    "tif": time_in_force
                }
            }

        # Add client order ID if provided
        if client_order_id:
            order["cloid"] = client_order_id

        # Build transaction
        transaction = {
            "action": {
                "type": "order",
                "orders": [order]
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature
        print(f"sending order tx: {transaction}")

        # Send request
        response = requests.post(
            f"{self.base_url}/order",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def place_multiple_orders(self, orders: List[Dict]) -> Dict:
        """
        Place multiple orders in a single transaction

        Args:
            orders: List of order dicts with keys:
                - symbol: Market symbol
                - is_buy: True for buy, False for sell
                - price: Order price
                - size: Order size
                - order_type: "limit" or "market"
                - time_in_force: "GTC", "IOC", or "ALO"
                - reduce_only: Optional, default False
                - client_order_id: Optional client order ID

        Returns:
            API response with order statuses

        Example:
            orders = [
                {
                    "symbol": "BTC-USD",
                    "is_buy": True,
                    "price": 99000.0,
                    "size": 0.1,
                    "order_type": "limit",
                    "time_in_force": "GTC"
                },
                {
                    "symbol": "ETH-USD",
                    "is_buy": False,
                    "price": 3000.0,
                    "size": 1.0,
                    "order_type": "limit",
                    "time_in_force": "GTC"
                }
            ]
            response = client.place_multiple_orders(orders)
        """
        if not self.signer:
            raise ValueError("Private key required for trading operations")

        # Build order objects
        order_objects = []
        for order_spec in orders:
            order = {
                "c": order_spec["symbol"],
                "b": order_spec["is_buy"],
                "px": order_spec["price"],
                "sz": order_spec["size"],
                "r": order_spec.get("reduce_only", False)
            }

            # Set order type
            if order_spec.get("order_type", "limit") == "market":
                order["t"] = {
                    "trigger": {
                        "is_market": True,
                        "triggerPx": 0.0
                    }
                }
            else:
                order["t"] = {
                    "limit": {
                        "tif": order_spec.get("time_in_force", "GTC")
                    }
                }

            # Add client order ID if provided
            if "client_order_id" in order_spec:
                order["cloid"] = order_spec["client_order_id"]

            order_objects.append(order)

        # Build transaction
        transaction = {
            "action": {
                "type": "order",
                "orders": order_objects
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

        # Send request
        response = requests.post(
            f"{self.base_url}/order",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def cancel_order(self, symbol: str, order_id: str) -> Dict:
        """
        Cancel a specific order

        Args:
            symbol: Market symbol (e.g., "BTC-USD")
            order_id: Order ID to cancel (base58)

        Returns:
            API response with cancellation status

        Example:
            response = client.cancel_order(
                symbol="BTC-USD",
                order_id="Fpa3oVuL3UzjNANAMZZdmrn6D1Zhk83GmBuJpuAWG51F"
            )
        """
        if not self.signer:
            raise ValueError("Private key required for trading operations")

        # Build transaction
        transaction = {
            "action": {
                "type": "cancel",
                "cancels": [
                    {
                        "c": symbol,
                        "oid": order_id
                    }
                ]
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

        # Send request
        response = requests.delete(
            f"{self.base_url}/order",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def cancel_multiple_orders(self, cancellations: List[Dict]) -> Dict:
        """
        Cancel multiple orders in a single transaction

        Args:
            cancellations: List of dicts with keys:
                - symbol: Market symbol
                - order_id: Order ID to cancel

        Returns:
            API response with cancellation statuses

        Example:
            cancellations = [
                {"symbol": "BTC-USD", "order_id": "order_id_1"},
                {"symbol": "ETH-USD", "order_id": "order_id_2"}
            ]
            response = client.cancel_multiple_orders(cancellations)
        """
        if not self.signer:
            raise ValueError("Private key required for trading operations")

        # Build cancellation objects
        cancel_objects = [
            {
                "c": cancel["symbol"],
                "oid": cancel["order_id"]
            }
            for cancel in cancellations
        ]

        # Build transaction
        transaction = {
            "action": {
                "type": "cancel",
                "cancels": cancel_objects
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

        # Send request
        response = requests.delete(
            f"{self.base_url}/order",
            json=transaction,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def cancel_all_orders(self, symbol: Optional[str] = None) -> Dict:
        """
        Cancel all orders (optionally for a specific symbol)

        Args:
            symbol: Optional market symbol to cancel orders for (None = all symbols)

        Returns:
            API response with cancellation status

        Example:
            # Cancel all orders on BTC-USD
            response = client.cancel_all_orders(symbol="BTC-USD")

            # Cancel all orders on all symbols
            response = client.cancel_all_orders()
        """
        if not self.signer:
            raise ValueError("Private key required for trading operations")

        # Build transaction
        transaction = {
            "action": {
                "type": "cancelall"
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Add symbol filter if provided
        if symbol:
            transaction["action"]["c"] = symbol

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

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
                "type": "updateusersettings",
                "settings": {
                    "m": leverage_settings
                }
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

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
                "type": "agentwalletcreation",
                "agent": {
                    "a": agent_pubkey,
                    "d": delete
                }
            },
            "account": self.signer.public_key,
            "signer": self.signer.public_key
        }

        # Sign transaction
        signature = self.signer.sign_transaction(transaction)
        transaction["signature"] = signature

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

    def request_faucet(self, user: Optional[str] = None) -> Dict:
        """
        Request testnet faucet funds (100,000 USDC)

        TESTNET ONLY - This endpoint is only available on testnet environments

        Args:
            user: Optional user public key (defaults to signer's public key)

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
        transaction = {
            "action": {
                "type": "faucet",
                "faucet": {
                    "u": target_user
                }
            },
            "account": target_user,
            "signer": self.signer.public_key
        }

        # Sign transaction
        tx = self.signer.sign_transaction(transaction)

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

if __name__ == "__main__":
    import os

    # Example 1: Read-only market data access (no private key needed)
    print("=== Read-only Market Data ===")
    base_url = "https://exchange-api2.bulk.trade/api/v1"
    client = BulkHttpClient(base_url=base_url, private_key=private_key)

    # Get exchange info
    exchange_info = client.get_exchange_info()
    print(f"Available symbols: {exchange_info.get('symbols', [])[:5]}...")

    # Get ticker
    ticker = client.get_ticker("BTC-USD")
    print(f"BTC-USD last price: ${ticker.get('lastPrice', 0):,.2f}")

    # Get orderbook
    orderbook = client.get_orderbook("BTC-USD", nlevels=5)
    print(f"Top bid: ${orderbook['bids'][0][0]:,.2f}")
    print(f"Top ask: ${orderbook['asks'][0][0]:,.2f}")

    # Get candles
    candles = client.get_klines("BTC-USD", "1h", limit=5)
    print(f"Latest candle close: ${candles[-1]['c']:,.2f}")

    # Example 2: Account queries (no private key needed)
    print("\n=== Account Queries (Unsigned) ===")
    test_user = "9J8TUdEWrrcADK913r1Cs7DdqX63VdVU88imfDzT1ypt"

    # Get full account
    account = client.get_full_account(test_user)
    if account:
        print(f"Account has {len(account.get('fullAccount', {}).get('positions', []))} positions")

    # Get open orders
    orders = client.get_open_orders(test_user)
    print(f"Found {len(orders)} open orders")

    # Example 3: Trading operations (requires private key)
    print("\n=== Trading Operations (Signed) ===")
    private_key = os.environ.get("BULK_PRIVATE_KEY")
    if private_key:
        trading_client = create_client(private_key=private_key)

        # Request faucet (testnet only)
        try:
            faucet_result = trading_client.request_faucet()
            print(f"Faucet request: {faucet_result}")
        except Exception as e:
            print(f"Faucet error (expected on mainnet): {e}")

        # Place a limit order (example - adjust price/size as needed)
        try:
            order_result = trading_client.place_order(
                symbol="BTC-USD",
                is_buy=True,
                price=50000.0,  # Far from market for safety
                size=0.001,
                order_type="limit",
                time_in_force="GTC"
            )
            print(f"Order placed: {order_result}")
        except Exception as e:
            print(f"Order placement: {e}")

        # Update leverage
        try:
            leverage_result = trading_client.update_leverage([
                ("BTC-USD", 5.0),
                ("ETH-USD", 3.0)
            ])
            print(f"Leverage updated: {leverage_result}")
        except Exception as e:
            print(f"Leverage update: {e}")
    else:
        print("Set BULK_PRIVATE_KEY environment variable for trading examples")