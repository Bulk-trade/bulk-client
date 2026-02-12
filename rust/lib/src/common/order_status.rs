use serde::{Deserialize};

/// Various order status
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum OrderStatus {
    // order state preplacement (would not be seen in events)
    Pending,
    // order has been placed
    Placed,
    // order is resting in the book
    Resting,
    // order is in book and has received some fills
    Working,
    // order was just modified
    Modified,
    // order fully filled and terminal
    Filled,
    // order partially filled and terminal
    PartiallyFilled,
    // order cancelled
    Cancelled,
    // order cancelled due to risk limit
    CancelledRiskLimit,
    // order was self crossing
    CancelledSelfCrossing,
    // order would not reduce position
    CancelledReduceOnly,
    // order expired without fill (IOC)
    CancelledIOC,
    // order is invalid
    RejectedInvalid,
    // order immediately rejected due to limits
    RejectedRiskLimit,
    // post-only order rejected due to crossing
    RejectedCrossing,
    // order rejected due to being a duplicate
    RejectedDuplicate,
}

#[allow(unused)]
impl OrderStatus {
    /// Determine if order status is terminal
    pub fn is_terminal(&self) -> bool {
        match self {
            OrderStatus::Pending => false,
            OrderStatus::Placed => false,
            OrderStatus::Resting => false,
            OrderStatus::Working => false,
            OrderStatus::Modified => true,
            OrderStatus::Filled => true,
            OrderStatus::PartiallyFilled => true,
            OrderStatus::Cancelled => true,
            OrderStatus::CancelledRiskLimit => true,
            OrderStatus::CancelledSelfCrossing => true,
            OrderStatus::CancelledReduceOnly => true,
            OrderStatus::CancelledIOC => true,
            OrderStatus::RejectedInvalid => true,
            OrderStatus::RejectedRiskLimit => true,
            OrderStatus::RejectedCrossing => true,
            OrderStatus::RejectedDuplicate => true,
        }
    }

    /// Indicate whether is a reject
    pub fn is_rejected(&self) -> bool {
        match self {
            OrderStatus::RejectedInvalid => true,
            OrderStatus::RejectedRiskLimit => true,
            OrderStatus::RejectedCrossing => true,
            OrderStatus::RejectedDuplicate => true,
            _ => false,
        }
    }
}

impl TryFrom<&str> for OrderStatus {
    type Error = String;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "pending" => Ok(OrderStatus::Pending),
            "placed" => Ok(OrderStatus::Placed),
            "resting" => Ok(OrderStatus::Resting),
            "working" => Ok(OrderStatus::Working),
            "modified" => Ok(OrderStatus::Modified),
            "filled" => Ok(OrderStatus::Filled),
            "partiallyFilled" => Ok(OrderStatus::PartiallyFilled),
            "canceled" => Ok(OrderStatus::Cancelled),
            "canceledRiskLimit" => Ok(OrderStatus::CancelledRiskLimit),
            "canceledSelfCrossing" => Ok(OrderStatus::CancelledSelfCrossing),
            "canceledReduceOnly" => Ok(OrderStatus::CancelledReduceOnly),
            "canceledIOC" => Ok(OrderStatus::CancelledIOC),
            "rejectedInvalid" => Ok(OrderStatus::RejectedInvalid),
            "rejectedRiskLimit" => Ok(OrderStatus::RejectedRiskLimit),
            "rejectedCrossing" => Ok(OrderStatus::RejectedCrossing),
            "rejectedDuplicate" => Ok(OrderStatus::RejectedDuplicate),
            _ => Err(format!("Unknown OrderStatus: {}", status)),
        }
    }
}

impl TryFrom<String> for OrderStatus {
    type Error = String;

    fn try_from(status: String) -> Result<Self, Self::Error> {
        Self::try_from(status.as_str())
    }
}

impl<'de> Deserialize<'de> for OrderStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <&str>::deserialize(deserializer)?;
        OrderStatus::try_from(s).map_err(serde::de::Error::custom)
    }
}