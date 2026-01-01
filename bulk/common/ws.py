import json
import websockets
import logging


class LoggingWebSocket:
    def __init__(self, ws, logger=None, formatted=False):
        self.logger = logger or logging.getLogger(__name__)
        self.ws = ws
        self.formatted = formatted

    async def recv(self):
        message = await self.ws.recv()
        if self.formatted:
            try:
                parsed = json.loads(message)
                self.logger.info(f"← Received:\n{json.dumps(parsed, indent=2)}")
            except:
                self.logger.info(f"← Received: {message}")
        else:
            self.logger.info(f"← Received: {message}")

        return message

    async def send(self, message):
        if self.formatted:
            try:
                parsed = json.loads(message)
                self.logger.info(f"→ Sending:\n{json.dumps(parsed, indent=2)}")
            except:
                self.logger.info(f"→ Sending: {message}")
        else:
            self.logger.info(f"→ Sending: {message}")
        return await self.ws.send(message)

    async def close(self):
        await self.ws.close()

    def __aiter__(self):
        """Support async iteration"""
        return self

    async def __anext__(self):
        """Receive next message in iteration"""
        try:
            return await self.recv()
        except websockets.exceptions.ConnectionClosed:
            raise StopAsyncIteration

    def __getattr__(self, name):
        # Delegate all other attributes to the wrapped websocket
        return getattr(self._ws, name)
