//! Numeric transaction schema supported by this client.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Transaction schema accepted by this client.
pub enum TransactionVersion {
    /// Fixed V13 action layout and numeric signing prefix.
    V13,
}

impl TransactionVersion {
    /// Version used for every newly constructed client transaction.
    pub const CURRENT: Self = Self::V13;

    /// Returns the numeric wire version.
    pub const fn number(self) -> u32 {
        match self {
            Self::V13 => 13,
        }
    }
}

impl Default for TransactionVersion {
    fn default() -> Self {
        Self::V13
    }
}

impl serde::Serialize for TransactionVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.number())
    }
}

impl<'de> serde::Deserialize<'de> for TransactionVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match <u32 as serde::Deserialize>::deserialize(deserializer)? {
            13 => Ok(Self::V13),
            version => Err(serde::de::Error::custom(format!(
                "unsupported transaction version {version}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TransactionVersion;

    #[test]
    fn only_explicit_v13_is_accepted() {
        assert_eq!(
            serde_json::to_string(&TransactionVersion::CURRENT).unwrap(),
            "13"
        );
        assert_eq!(
            serde_json::from_str::<TransactionVersion>("13").unwrap(),
            TransactionVersion::V13
        );
        for version in ["11", "12", "null", "\"13\""] {
            assert!(serde_json::from_str::<TransactionVersion>(version).is_err());
        }
        let tx = serde_json::json!({"version":13,"actions":[],"nonce":42,"account":"11111111111111111111111111111111","signer":"11111111111111111111111111111111","signature":"1111111111111111111111111111111111111111111111111111111111111111"});
        let tx: crate::transaction::Transaction = serde_json::from_value(tx).unwrap();
        assert!(serde_json::to_string(&tx)
            .unwrap()
            .starts_with(r#"{"version":13,"actions":"#));
        let bytes = bincode::serialize(&tx).unwrap();
        assert_eq!(&bytes[..4], &13u32.to_le_bytes());
        assert!(bincode::deserialize::<crate::transaction::Transaction>(&bytes[4..]).is_err());
        let mut missing = serde_json::to_value(&tx).unwrap();
        missing.as_object_mut().unwrap().remove("version");
        assert!(serde_json::from_value::<crate::transaction::Transaction>(missing).is_err());
    }
}
