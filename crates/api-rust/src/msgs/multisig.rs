use serde::{Deserialize, Serialize};
use solana_keypair::Pubkey;
use crate::transaction::{Action, ActionMeta};

/// Create multi-sig account
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMultisig {
    #[serde(default)]
    pub signers: Vec<Pubkey>,
    pub threshold: u32,
    #[serde(default)]
    pub time_lock_secs: u32,
    #[serde(default = "default_proposal_lifetime_secs")]
    pub proposal_lifetime_secs: u32,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Create multi-sig proposed action(s)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigPropose {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(rename = "a", default)]
    pub actions: Vec<Action>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Multi-sig approval
/// - one signer approves proposal
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigApprove {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(rename = "p")]
    pub proposal_id: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Multi-sig reject
/// - one signer rejects proposal
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigReject {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(rename = "p")]
    pub proposal_id: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Multi-sig cancel
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigCancel {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(rename = "p")]
    pub proposal_id: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Multi-sig execute
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigExecute {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(rename = "p")]
    pub proposal_id: u64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Update multi-sig requirements
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMultisigPolicy {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub multisig: Pubkey,
    #[serde(default = "default_signers")]
    pub signers: Vec<Pubkey>,
    pub threshold: u32,
    #[serde(default)]
    pub time_lock_secs: u32,
    #[serde(default = "default_proposal_lifetime_secs")]
    pub proposal_lifetime_secs: u32,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_signers() -> Vec<Pubkey> {
    Vec::new()
}
fn default_proposal_lifetime_secs() -> u32 {
    7 * 24 * 3600
}

