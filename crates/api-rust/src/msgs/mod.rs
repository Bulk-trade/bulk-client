pub mod md;
pub mod subscription;
pub mod account;
pub mod responses;
pub mod meta;
pub mod oracle;
pub mod order;
pub mod conditional;

pub use md::*;
pub use subscription::*;
pub use account::*;
pub use responses::*;
pub use meta::*;
pub use oracle::*;
pub use order::*;

pub(crate) mod serde_hash {
    use serde::{Deserialize, Deserializer, Serializer};
    use solana_hash::Hash;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Hash, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            serializer.serialize_bytes(val.as_bytes())
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Hash::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            Hash::deserialize(deserializer)
        }
    }
}

pub(crate) mod serde_pubkey {
    use serde::{Deserialize, Deserializer, Serializer};
    use solana_pubkey::Pubkey;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Pubkey, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            serializer.serialize_bytes(val.as_array())
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Pubkey, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Pubkey::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            Pubkey::deserialize(deserializer)
        }
    }
}

pub(crate) mod serde_signature {
    use serde::{Deserialize, Deserializer, Serializer};
    use solana_signature::Signature;
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(val: &Signature, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&val.to_string())
        } else {
            serializer.serialize_bytes(val.as_array())
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Signature, D::Error> {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            Signature::from_str(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
        } else {
            Signature::deserialize(deserializer)
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
