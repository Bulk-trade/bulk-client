import sys
import asyncio
from mm.bmm.BinanceMM import main

if __name__ == "__main__":
    sys.exit(asyncio.run(main()))