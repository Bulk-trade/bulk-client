use crate::msgs::sig_bytes;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use solana_keypair::Pubkey;
use crate::transaction::ActionMeta;

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
    pub node_id: u16,
    pub wall_clock_ns: u64,
    pub since_commit_us: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
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
    pub node_id: u16,
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

    #[serde(skip)]
    pub meta: ActionMeta,
}