pub mod account;
pub mod conditional;
pub mod history;
pub mod liquidator;
pub mod md;
pub mod meta;
pub mod multisig;
pub mod oracle;
pub mod order;
pub mod responses;
pub mod risk;
pub mod subaccounts;
pub mod subscription;

pub use account::*;
pub use history::*;
pub use md::*;
pub use meta::*;
pub use oracle::*;
pub use order::*;
pub use responses::*;
pub use subscription::*;

pub(crate) mod sig_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(sig: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        sig.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let v = <Vec<u8>>::deserialize(d)?;
        v.try_into()
            .map_err(|_| serde::de::Error::custom("expected 64-byte signature"))
    }
}

pub(crate) mod serde_hash {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use solana_hash::Hash;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Hash, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            val.as_bytes().serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Hash::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            let bytes = <Vec<u8>>::deserialize(deserializer)?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("expected 32 bytes for Hash"))?;
            Ok(Hash::new_from_array(arr))
        }
    }
}

pub(crate) mod serde_pubkey {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use solana_pubkey::Pubkey;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Pubkey, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            val.as_array().serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Pubkey, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Pubkey::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            let bytes = <Vec<u8>>::deserialize(deserializer)?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("expected 32 bytes for Pubkey"))?;
            Ok(Pubkey::from(arr))
        }
    }
}

pub(crate) mod serde_signature {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use solana_signature::Signature;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Signature, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            val.as_array().serialize(serializer)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Signature, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Signature::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            let bytes = <Vec<u8>>::deserialize(deserializer)?;
            let arr: [u8; 64] = bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("expected 64 bytes for Signature"))?;
            Ok(Signature::from(arr))
        }
    }
}

pub(crate) mod fixed_point {
    use serde::{Deserialize, Deserializer, Serializer};

    const SCALE: f64 = 1e8;

    pub fn serialize<S: Serializer>(val: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_f64(*val)
        } else {
            let fixed = (val * SCALE).round() as u64;
            serializer.serialize_u64(fixed)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        if deserializer.is_human_readable() {
            f64::deserialize(deserializer)
        } else {
            let fixed = u64::deserialize(deserializer)?;
            Ok(fixed as f64 / SCALE)
        }
    }
}

pub(crate) mod opt_fixed_point {
    use serde::de::Visitor;
    use serde::{de, Deserialize, Deserializer, Serializer};
    use std::fmt;

    const SCALE: f64 = 1e8;

    pub fn serialize<S: Serializer>(val: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error> {
        match val {
            None => serializer.serialize_none(),
            Some(v) => {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&v.to_string())
                } else {
                    let fixed = (v * SCALE).round() as u64;
                    serializer.serialize_some(&fixed)
                }
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<f64>, D::Error> {
        deserializer.deserialize_any(OptF64Visitor)
    }

    struct OptF64Visitor;

    impl<'de> Visitor<'de> for OptF64Visitor {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an optional f64 as a string, float, integer, or null/absent")
        }

        // JSON null or binary None tag
        fn visit_none<E: de::Error>(self) -> Result<Option<f64>, E> {
            Ok(None)
        }

        // serde unit type (e.g. MessagePack nil)
        fn visit_unit<E: de::Error>(self) -> Result<Option<f64>, E> {
            Ok(None)
        }

        // Binary Some(T) — inner deserializer carries the fixed-point u64
        fn visit_some<D2: Deserializer<'de>>(
            self,
            deserializer: D2,
        ) -> Result<Option<f64>, D2::Error> {
            if deserializer.is_human_readable() {
                // Shouldn't normally reach here for JSON, but be safe
                deserializer.deserialize_any(OptF64Visitor)
            } else {
                let fixed = u64::deserialize(deserializer)?;
                Ok(Some(fixed as f64 / SCALE))
            }
        }

        fn visit_f64<E: de::Error>(self, val: f64) -> Result<Option<f64>, E> {
            Ok(Some(val))
        }

        fn visit_f32<E: de::Error>(self, val: f32) -> Result<Option<f64>, E> {
            Ok(Some(val as f64))
        }

        fn visit_u64<E: de::Error>(self, val: u64) -> Result<Option<f64>, E> {
            Ok(Some(val as f64))
        }

        fn visit_i64<E: de::Error>(self, val: i64) -> Result<Option<f64>, E> {
            Ok(Some(val as f64))
        }

        fn visit_str<E: de::Error>(self, val: &str) -> Result<Option<f64>, E> {
            val.parse::<f64>()
                .map(Some)
                .map_err(|_| E::custom(format!("invalid f64 string: \"{val}\"")))
        }

        fn visit_string<E: de::Error>(self, val: String) -> Result<Option<f64>, E> {
            self.visit_str(&val)
        }
    }
}
