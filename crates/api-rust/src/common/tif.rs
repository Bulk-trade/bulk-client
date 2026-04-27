use std::str::FromStr;
use eyre::bail;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde::de::Error;

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TimeInForce {
    GTC = 0,
    IOC = 1,
    ALO = 2,
}

impl From<u8> for TimeInForce {
    fn from(value: u8) -> TimeInForce {
        match value {
            0 => TimeInForce::GTC,
            1 => TimeInForce::IOC,
            2 => TimeInForce::ALO,
            _ => TimeInForce::GTC,
        }
    }
}

impl FromStr for TimeInForce {
    type Err = eyre::Error;

    fn from_str(s: &str) -> eyre::Result<Self> {
        match s.to_uppercase().as_str() {
            "GTC" => Ok(TimeInForce::GTC),
            "IOC" => Ok(TimeInForce::IOC),
            "ALO" | "POSTONLY" => Ok(TimeInForce::ALO),
            _ => bail!(
                "unknown time-in-force '{s}'\n  → expected GTC, IOC, ALO"
            ),
        }
    }
}


impl Default for TimeInForce {
    fn default() -> TimeInForce {
        TimeInForce::GTC
    }
}

impl Serialize for TimeInForce {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.serialize_str(match self {
                TimeInForce::GTC => "GTC",
                TimeInForce::IOC => "IOC",
                TimeInForce::ALO => "ALO",
            })
        } else {
            s.serialize_u32(*self as u32)
        }
    }
}

impl<'de> Deserialize<'de> for TimeInForce {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if d.is_human_readable() {
            let s = String::deserialize(d)?;
            match s.as_str() {
                "GTC" | "gtc" => Ok(TimeInForce::GTC),
                "IOC" | "ioc" => Ok(TimeInForce::IOC),
                "ALO" | "alo" | "postOnly" => Ok(TimeInForce::ALO),
                _ => Err(de::Error::unknown_variant(&s, &["GTC", "IOC", "ALO"])),
            }
        } else {
            let v = u32::deserialize(d)? as u8;
            Self::try_from(v).map_err(|_| {
                D::Error::invalid_value(serde::de::Unexpected::Unsigned(v as u64), &"0..=2")
            })
        }
    }
}

