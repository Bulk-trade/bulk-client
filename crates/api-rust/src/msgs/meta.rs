use crate::msgs::sig_bytes;
use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::{
    fmt,
    ops::{BitOr, BitOrAssign},
    sync::Arc,
};

/// Per-market configuration returned by the exchange.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct MarketInfo {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub status: String,
    pub price_precision: u32,
    pub size_precision: u32,
    pub tick_size: f64,
    pub lot_size: f64,
    pub min_notional: f64,
    pub max_leverage: f64,
    pub order_types: Vec<String>,
    pub time_in_forces: Vec<String>,
}

/// Beacon tx
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Beacon {
    pub epoch: u32,
    pub wall_clock_ns: u64,
    pub since_commit_us: u64,
    pub per_peer_origination: Vec<(Pubkey, u64)>,
    pub row_unattributed: u64,
    pub per_peer_participation: Vec<(Pubkey, u32)>,
    pub participation_total_rounds: u32,
    pub stake_announce: Option<StakeAnnounce>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Solana stake projected onto the reward roster for a beacon epoch.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StakeAnnounce {
    pub stake_epoch: u64,
    pub roster_hash: [u8; 32],
    pub values: Vec<u64>,
}

/// WarmJoin protocol: a validator announces it has caught up and is ready to vote.
///
/// System-internal transaction (like Beacon). Not a user order.
/// Authentication is via the BulkTransaction signer — verified against the
/// preconfigured validator pubkey map before consensus processes the join.
///
/// `committed_round` is the node's last committed round at emission time.
/// Re-emitted join TXs naturally hash differently, preventing dedup stalls.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Join {
    pub committed_round: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Add new market tx
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMarket {
    #[serde(rename = "c")]
    pub symbol: Arc<str>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Administrative market operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarketAction {
    Open,
    Suspend,
    Close,
}

/// Adjusts the operating state of an existing market.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketAdmin {
    pub symbol: Arc<str>,
    pub action: MarketAction,
    #[serde(default, with = "crate::msgs::opt_fixed_point")]
    pub price: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Bitmask selecting which publishers may update an instrument's oracle price.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OracleSource(u8);

#[allow(non_upper_case_globals)]
impl OracleSource {
    /// Accept Pyth and Bulk publishers (legacy name).
    pub const Both: Self = Self(0b011);
    /// Accept Pyth oracle prices.
    pub const Pyth: Self = Self(0b001);
    /// Accept globally authorized Bulk publishers.
    pub const Bulk: Self = Self(0b010);
    /// Accept instrument-authorized BIP publishers.
    pub const BIP: Self = Self(0b100);
    /// Accept every publisher class.
    pub const All: Self = Self(Self::Pyth.0 | Self::Bulk.0 | Self::BIP.0);

    /// Creates a source mask when every bit is recognized and at least one source is selected.
    ///
    /// # Arguments
    /// * `bits` - Packed source bits.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits != 0 && bits & !Self::All.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Returns the packed source bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Returns whether every source in `other` is enabled.
    ///
    /// # Arguments
    /// * `other` - Source subset to test.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether this source configuration accepts Pyth prices.
    pub const fn accepts_pyth(self) -> bool {
        self.contains(Self::Pyth)
    }

    /// Returns whether this source configuration accepts global Bulk publishers.
    pub const fn accepts_bulk(self) -> bool {
        self.contains(Self::Bulk)
    }

    /// Returns whether this source configuration accepts instrument-authorized BIP publishers.
    pub const fn accepts_bip(self) -> bool {
        self.contains(Self::BIP)
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        [
            (Self::Pyth, "Pyth"),
            (Self::Bulk, "Bulk"),
            (Self::BIP, "BIP"),
        ]
        .into_iter()
        .filter_map(move |(source, name)| self.contains(source).then_some(name))
    }

    fn named(name: &str) -> Option<Self> {
        match name {
            "Pyth" | "pyth" => Some(Self::Pyth),
            "Bulk" | "bulk" => Some(Self::Bulk),
            "BIP" | "bip" => Some(Self::BIP),
            "All" | "all" => Some(Self::All),
            "Both" | "both" => Some(Self::Pyth | Self::Bulk),
            "BulkOrBIP" | "bulkOrBip" => Some(Self::Bulk | Self::BIP),
            _ => None,
        }
    }
}

impl Default for OracleSource {
    fn default() -> Self {
        Self::Pyth | Self::Bulk
    }
}

impl BitOr for OracleSource {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for OracleSource {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl fmt::Debug for OracleSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.names()).finish()
    }
}

impl serde::Serialize for OracleSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let names = self.names().collect::<Vec<_>>();
            if let [name] = names.as_slice() {
                serializer.serialize_str(name)
            } else {
                serde::Serialize::serialize(&names, serializer)
            }
        } else {
            serializer.serialize_u8(self.bits())
        }
    }
}

impl<'de> serde::Deserialize<'de> for OracleSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(OracleSourceVisitor)
        } else {
            let bits = <u8 as serde::Deserialize>::deserialize(deserializer)?;
            Self::from_bits(bits).ok_or_else(|| {
                serde::de::Error::custom(format!("invalid oracle source mask {bits:#05b}"))
            })
        }
    }
}

struct OracleSourceVisitor;

impl<'de> serde::de::Visitor<'de> for OracleSourceVisitor {
    type Value = OracleSource;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an oracle source name or a non-empty array of source names")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        OracleSource::named(value)
            .ok_or_else(|| E::custom(format!("unknown oracle source {value:?}")))
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut sources = OracleSource(0);
        while let Some(name) = sequence.next_element::<String>()? {
            let source = OracleSource::named(&name).ok_or_else(|| {
                serde::de::Error::custom(format!("unknown oracle source {name:?}"))
            })?;
            sources |= source;
        }
        OracleSource::from_bits(sources.bits())
            .ok_or_else(|| serde::de::Error::custom("oracle source array cannot be empty"))
    }
}

/// Administrative oracle-source configuration for an instrument.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone, Debug)]
pub struct PricingAdmin {
    /// Oracle instrument, such as `BTC`.
    pub instrument: Arc<str>,
    /// Publisher classes accepted for subsequent oracle updates.
    ///
    /// JSON accepts one source name or an array of source names.
    #[serde(rename = "oracleSource", alias = "source")]
    pub oracle_source: OracleSource,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Opaque wrapper for special tx
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpaqueAction {
    pub payload: Vec<u8>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// A single admin's ed25519 signature over the `UpdateValidatorSet` message digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminSignature {
    pub admin_index: u8,
    #[serde(with = "sig_bytes")]
    pub signature: [u8; 64],
}

/// Delta-form admin-signed validator-set mutation.
///
/// Adds already in the set and removes not in the set are no-ops. A pubkey
/// appearing in both `added` and `removed` rejects the tx. `version` must
/// equal the current `ValidatorSet` version + 1.
///
/// Threshold-signed by `ADMIN_THRESHOLD` of `ADMIN_PUBKEYS`. Verification
/// and activation live in `bulk_consensus_proto::admin_txn`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateValidatorSet {
    pub added: Vec<Pubkey>,
    pub removed: Vec<Pubkey>,
    pub version: u64,
    pub admin_sigs: Vec<AdminSignature>,
    #[serde(default)]
    pub tier_changes: Vec<(Pubkey, u8)>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_sources_use_sdk_masks_and_accept_legacy_json() {
        for bits in 1..=7 {
            let source = OracleSource::from_bits(bits).unwrap();
            assert_eq!(bincode::serialize(&source).unwrap(), [bits]);
            assert_eq!(
                bincode::deserialize::<OracleSource>(&[bits]).unwrap(),
                source
            );
            assert_eq!(
                serde_json::from_value::<OracleSource>(serde_json::to_value(source).unwrap())
                    .unwrap(),
                source
            );
        }
        for bits in [0, 8, 255] {
            assert!(OracleSource::from_bits(bits).is_none());
            assert!(bincode::deserialize::<OracleSource>(&[bits]).is_err());
        }
        for invalid in [r#"[]"#, r#"["unknown"]"#, r#"0"#] {
            assert!(serde_json::from_str::<OracleSource>(invalid).is_err());
        }
        let legacy: PricingAdmin =
            serde_json::from_str(r#"{"instrument":"BTC","source":"both"}"#).unwrap();
        assert_eq!(
            legacy.oracle_source,
            OracleSource::Pyth | OracleSource::Bulk
        );
        assert_eq!(
            serde_json::to_value(legacy).unwrap()["oracleSource"],
            serde_json::json!(["Pyth", "Bulk"])
        );
        assert_eq!(
            serde_json::from_str::<OracleSource>(r#""bulkOrBip""#).unwrap(),
            OracleSource::Bulk | OracleSource::BIP
        );
    }

    #[test]
    fn beacon_and_join_use_current_sdk_shape() {
        let beacon = Beacon {
            epoch: 7,
            wall_clock_ns: 11,
            since_commit_us: 13,
            per_peer_origination: Vec::new(),
            row_unattributed: 17,
            per_peer_participation: Vec::new(),
            participation_total_rounds: 19,
            stake_announce: None,
            meta: ActionMeta::default(),
        };
        let join = Join {
            committed_round: 23,
            meta: ActionMeta::default(),
        };

        assert_eq!(
            serde_json::to_value(beacon).expect("beacon should serialize"),
            serde_json::json!({
                "epoch": 7,
                "wall_clock_ns": 11,
                "since_commit_us": 13,
                "per_peer_origination": [],
                "row_unattributed": 17,
                "per_peer_participation": [],
                "participation_total_rounds": 19,
                "stake_announce": null
            })
        );
        assert_eq!(
            serde_json::to_value(join).expect("join should serialize"),
            serde_json::json!({"committed_round": 23})
        );
    }

    #[test]
    fn validator_set_defaults_omitted_tier_changes() {
        let update: UpdateValidatorSet = serde_json::from_value(serde_json::json!({
            "added": [],
            "removed": [],
            "version": 1,
            "admin_sigs": []
        }))
        .expect("validator set update should deserialize");

        assert!(update.tier_changes.is_empty());
    }
}
