#!/usr/bin/env python3
"""
Test Order ID Pre-computation
"""

import asyncio
import json
import logging
import os
import sys
import time
from pathlib import Path

from bulk import BulkHttpClient
from bulk.common import Side, TimeInForce, Topic
from bulk.common.signer import TransactionSigner
from bulk.messages.trade import LimitOrder, OrderResponse
from bulk.api import BulkWebSocketClient


# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

def test_order_id1():
    order1 = LimitOrder(
        side=Side.BUY,
        symbol="ETH-USD",
        size=22.142599999999998,
        price=2002.25,
        time_in_force=TimeInForce.GTC,
        pubkey="7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5",
        nonce=1772806443300040,
    )
    expected = "51RuEpaNYLMrTKTvf5vEDeMq2LWMSdmxbeUTzq1Efgtw"
    oid = order1.order_id()
    assert oid == expected

if __name__ == "__main__":
    test_order_id1()