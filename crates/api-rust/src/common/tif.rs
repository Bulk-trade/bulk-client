use eyre::bail;
use serde::de::Error;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::str::FromStr;

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TimeInForce {
    GTC = 0,
    IOC = 1,
    ALO = 2,
    ALO_SLIDE = 3,
    ALO_JOIN = 4,
}

impl From<u8> for TimeInForce {
    fn from(value: u8) -> TimeInForce {
        match value {
            0 => TimeInForce::GTC,
            1 => TimeInForce::IOC,
            2 => TimeInForce::ALO,
            3 => TimeInForce::ALO_SLIDE,
            4 => TimeInForce::ALO_JOIN,
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
            "ALO_SLIDE" | "POSTONLYSLIDE" => Ok(TimeInForce::ALO_SLIDE),
            "ALO_JOIN" | "POSTONLYJOIN" => Ok(TimeInForce::ALO_JOIN),
            _ => bail!(
                "unknown time-in-force '{s}'\n  → expected GTC, IOC, ALO, ALO_SLIDE, ALO_JOIN"
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
                TimeInForce::ALO_SLIDE => "ALO_SLIDE",
                TimeInForce::ALO_JOIN => "ALO_JOIN",
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
                "ALO_SLIDE" | "alo_slide" | "postOnlySlide" => Ok(TimeInForce::ALO_SLIDE),
                "ALO_JOIN" | "alo_join" | "postOnlyJoin" => Ok(TimeInForce::ALO_JOIN),
                _ => Err(de::Error::unknown_variant(
                    &s,
                    &["GTC", "IOC", "ALO", "ALO_SLIDE", "ALO_JOIN"],
                )),
            }
        } else {
            let v = u32::deserialize(d)? as u8;
            Self::try_from(v).map_err(|_| {
                D::Error::invalid_value(serde::de::Unexpected::Unsigned(v as u64), &"0..=4")
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alo_slide_and_join_round_trip_with_stable_wire_values() {
        for (tif, value, json) in [
            (TimeInForce::ALO_SLIDE, 3u32, "\"ALO_SLIDE\""),
            (TimeInForce::ALO_JOIN, 4u32, "\"ALO_JOIN\""),
        ] {
            assert_eq!(serde_json::to_string(&tif).unwrap(), json);
            assert_eq!(serde_json::from_str::<TimeInForce>(json).unwrap(), tif);
            assert_eq!(bincode::serialize(&tif).unwrap(), value.to_le_bytes());
            assert_eq!(
                bincode::deserialize::<TimeInForce>(&value.to_le_bytes()).unwrap(),
                tif
            );
        }
    }

    #[test]
    fn alo_slide_and_join_accept_api_and_cli_aliases() {
        for (input, expected) in [
            ("ALO_SLIDE", TimeInForce::ALO_SLIDE),
            ("postOnlySlide", TimeInForce::ALO_SLIDE),
            ("ALO_JOIN", TimeInForce::ALO_JOIN),
            ("postOnlyJoin", TimeInForce::ALO_JOIN),
        ] {
            assert_eq!(input.parse::<TimeInForce>().unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<TimeInForce>(&format!("\"{input}\"")).unwrap(),
                expected
            );
        }
    }
}
