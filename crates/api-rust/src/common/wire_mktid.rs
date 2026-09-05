use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// SDK-independent market identifier matching the binary representation of SDK `MktId`.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct WireMktId(pub u64);

impl FromStr for WireMktId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let uuid = if let Some(hex) = value.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)?
        } else {
            value.parse()?
        };
        Ok(Self(uuid))
    }
}

impl fmt::Display for WireMktId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:016x}", self.0)
    }
}

impl Serialize for WireMktId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_string())
        } else {
            serializer.serialize_u64(self.0)
        }
    }
}

impl<'de> Deserialize<'de> for WireMktId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let value = String::deserialize(deserializer)?;
            value
                .parse()
                .map_err(|error| de::Error::custom(format!("invalid numeric market ID: {error}")))
        } else {
            u64::deserialize(deserializer).map(Self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_encoding_matches_sdk_mktid_u64_wire_format() {
        let id = WireMktId(0x0102_0304_0506_0708);

        assert_eq!(
            bincode::serialize(&id).expect("serialize market ID"),
            0x0102_0304_0506_0708_u64.to_le_bytes()
        );
    }

    #[test]
    fn human_readable_input_accepts_decimal_and_hex_ids() {
        assert_eq!(
            serde_json::from_str::<WireMktId>(r#""42""#).unwrap(),
            WireMktId(42)
        );
        assert_eq!(
            serde_json::from_str::<WireMktId>(r#""0x2a""#).unwrap(),
            WireMktId(42)
        );
    }
}
