"""Compatibility import path for the Bulk Python client.

Preferred import path is `bulk_client` to match the PyPI package name.
`bulk_api` remains available for backward compatibility.
"""

from bulk_api import BulkHttpClient, BulkWebSocketClient, FastOrderBook, Topic

__all__ = ["BulkHttpClient", "BulkWebSocketClient", "FastOrderBook", "Topic"]
