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
    GTC = "GTC"
    # Immediate or Cancel
    IOC = "IOC"
    # Add Liquidity Only (i.e. Post-Only)
    ALO = "ALO"

    def __str__(self):
        """Convert to string for API"""
        return self.name


## Order status
class OrderStatus(Enum):
    ERROR = None
    NONE = 0
    RESTING = 1
    WORKING = 2
    FILLED = 3
    PARTIALLY_FILLED = 4
    CANCELLED = 5
    CANCELLED_RISKLIMIT = 6
    CANCELLED_SELFCROSSING = 7
    CANCELLED_IOC = 8
    CANCELLED_REDUCEONLY = 9
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

    def is_cancelled(self) -> bool:
        """Determine if the order is cancelled or not"""
        return self.value >= OrderStatus.CANCELLED.value and self.value <= OrderStatus.CANCELLED_REDUCEONLY.value

    def is_placed(self) -> bool:
        """Determine if the order was placed or not"""
        return self.value > OrderStatus.NONE.value and self.value < OrderStatus.CANCELLED.value

    @classmethod
    def from_string(cls, s: str) -> 'OrderStatus':
        """Construct an OrderStatus enum from a string"""
        match s:
            case "resting":
                return OrderStatus.RESTING
            case "placed":
                return OrderStatus.RESTING
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

