import asyncio
import time

import websockets
import json

async def send_order():
    uri = "wss://exchange-wss.bulk.trade"
    async with websockets.connect(uri) as websocket:
        msg = {"method": "post", "request": {"type": "action", "payload": {
            "action": {
                "type": "order",
                "orders": [{
                    "cancelAll": {"c": ["BTC-USD"]}}
                ],
                "nonce": time.time_ns() % 1000000000
            },
            "account": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5",
            "signer": "7DHvrCZMMLZ2ovNfKaGpvJZXAQyydbTz6dM7w7qXtzX5",
            "signature": "5bnQYX3Rg6QZ3nZFi8Sx2A4HvHuNMfn9W23hs652aqKEjiebRVdLkdKPVkSWU5WuP26PQVfsuCXPTccc8qsHpDKr"}}, "id": 1}

        sjson = json.dumps(msg)
        print(f"sending: {sjson}")

        await websocket.send(sjson)
        response = await websocket.recv()
        print(f"received: {response}")


asyncio.run(send_order())
