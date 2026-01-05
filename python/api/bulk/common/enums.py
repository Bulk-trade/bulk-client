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
    RESTING = 1
    WORKING = 2
    FILLED = 3
    PARTIALLY_FILLED = 4
    CANCELLED = 5
    CANCELLED_RISKLIMIT = 6
    CANCELLED_SELFCROSSING = 7
    CANCELLED_IOC = 8
    CANCELLED_REDUCE_ONLY = 9
    REJECTED_CROSSING = 10
    REJECTED_DUPLICATE = 11
    REJECTED_RISKLIMIT = 12
    REJECTED_INVALID = 13

    def __str__(self):
        """Convert to string for API"""
        return self.name

    def __str__(self):
        """Convert to string for API"""
        return self.name.lower()

    def is_terminal(self) -> bool:
        """Determine if the order is terminal or not"""
        return self.value > OrderStatus.WORKING.value

    @classmethod
    def from_string(cls, s: str) -> 'OrderStatus':
        """Construct an OrderStatus enum from a string"""
        match s:
            case "placed":
                return OrderStatus.PLACED
            case "working":
                return OrderStatus.WORKING
            case "filled":
                return OrderStatus.FILLED
            case "partiallyFilled":
                return OrderStatus.PARTIALLY_FILLED
            case "cancelled":
                return OrderStatus.CANCELLED
            case "cancelledRiskLimit":
                return OrderStatus.CANCELLED_RISKLIMIT
            case "cancelledSelfCrossing":
                return OrderStatus.CANCELLED_SELFCROSSING
            case "cancelledReduceOnly":
                return OrderStatus.CANCELLED_REDUCEONLY
            case "cancelledIOC":
                return OrderStatus.CANCELLED_IOC
            case "rejectedCrossing":
                return OrderStatus.REJECTED_CROSSING
            case "rejectedDuplicate":
                return OrderStatus.REJECTED_DUPLICATE
            case "rejectedRiskLimit":
                return OrderStatus.REJECTED_RISKLIMIT
            case "rejectedInvalid":
                return OrderStatus.REJECTED_INVALID
            case _:
                raise ValueError(f"Unknown order status {s}")

