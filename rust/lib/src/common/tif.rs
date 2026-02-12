use std::fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC = 0,
    IOC = 1,
    ALO = 2,
}

impl Display for TimeInForce {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeInForce::GTC => write!(f, "GTC"),
            TimeInForce::IOC => write!(f, "IOC"),
            TimeInForce::ALO => write!(f, "ALO"),
        }
    }
}