use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum TimeInForce {
    GTC,
    IOC,
    ALO,
}

impl Into<u8> for TimeInForce {
    fn into(self) -> u8 {
        match self {
            TimeInForce::GTC => 0,
            TimeInForce::IOC => 1,
            TimeInForce::ALO => 2,
        }
    }
}