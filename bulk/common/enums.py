from enum import Enum

## Buy or Sell direction
class Side(Enum):
    BUY = 1
    SELL = 0

    def __str__(self):
        """Convert to string for display"""
        return self.name

    def to_bool(self) -> bool:
        """Convert to `is_buy` bool for API"""
        return self == Side.BUY


## Time in force atrribute
class TimeInForce(Enum):
    # Good till closed
    GTC = 0
    # Immediate or Cancel
    IOC = 1
    # Add Liquidity Only (i.e. Post-Only)
    ALO = 2

    def __str__(self):
        """Convert to string for API"""
        return self.name


## Order status
class OrderStatus(Enum):
    NONE = 0
    PLACED = 1
    FILLED = 2
    PARTIALLY_FILLED = 3
    CANCELLED = 4
    REJECTED = 5

    def __str__(self):
        """Convert to string for API"""
        return self.name.lower()

    def is_terminal(self) -> bool:
        """Determine if the order is terminal or not"""
        return self.value > OrderStatus.PLACED.value

    @classmethod
    def from_string(cls, s: str) -> 'OrderStatus':
        """Construct an OrderStatus enum from a string"""
        return OrderStatus[s.upper()]

