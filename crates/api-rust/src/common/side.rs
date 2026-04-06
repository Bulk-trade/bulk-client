use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};

/// Buy / Sell
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Default, IntoPrimitive, FromPrimitive, Hash, Ord, PartialOrd,
)]
#[repr(u8)]
pub enum Side {
    #[default]
    Buy = 0,
    Sell = 1,
}


impl Side {
    pub fn dir(&self) -> f64 {
        match self {
            Side::Buy => 1.0,
            Side::Sell => -1.0,
        }
    }
}

/// Formatting
impl Display for Side {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "Buy"),
            Side::Sell => write!(f, "Sell"),
        }
    }
}

impl Into<f64> for Side {
    fn into(self) -> f64 {
        match self {
            Side::Buy => 1.0,
            Side::Sell => -1.0,
        }
    }
}

impl From<bool> for Side {
    fn from(v: bool) -> Self {
        if v {
            Side::Buy
        } else {
            Side::Sell
        }
    }
}

// Custom serialization to store as u8 (binary) or String (human-readable)
impl Serialize for Side {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(*self == Side::Buy)
    }
}

// Custom deserialization from u8 (binary) or String (human-readable)
impl<'de> Deserialize<'de> for Side {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = bool::deserialize(deserializer)?;
        Ok(if value { Side::Buy } else { Side::Sell })
    }
}
