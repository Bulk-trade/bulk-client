use crate::transaction::{Action, ActionMeta};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

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
    /// Optional proposal lifetime in seconds, bounded by the multisig policy maximum.
    #[serde(default, rename = "l")]
    pub proposal_lifetime_secs: Option<u32>,

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
    #[serde(default)]
    pub signers: Option<Vec<Pubkey>>,
    #[serde(default)]
    pub threshold: Option<u32>,
    #[serde(default)]
    pub time_lock_secs: Option<u32>,
    #[serde(default)]
    pub proposal_lifetime_secs: Option<u32>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

fn default_proposal_lifetime_secs() -> u32 {
    7 * 24 * 3600
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multisig_proposal_lifetime_uses_sdk_wire_field() {
        let proposal = MultisigPropose {
            multisig: Pubkey::default(),
            actions: Vec::new(),
            proposal_lifetime_secs: None,
            meta: ActionMeta::default(),
        };

        let value = serde_json::to_value(&proposal).expect("serialize proposal");
        assert_eq!(value.get("l"), Some(&serde_json::Value::Null));

        let without_lifetime = serde_json::json!({
            "m": Pubkey::default().to_string(),
            "a": []
        });
        let decoded: MultisigPropose =
            serde_json::from_value(without_lifetime).expect("deserialize proposal");
        assert_eq!(decoded.proposal_lifetime_secs, None);
    }

    #[test]
    fn update_multisig_policy_defaults_omitted_changes_to_none() {
        let multisig = Pubkey::new_unique();
        let update: UpdateMultisigPolicy = serde_json::from_value(serde_json::json!({
            "m": multisig.to_string()
        }))
        .expect("partial multisig update should parse");

        assert_eq!(update.multisig, multisig);
        assert_eq!(update.signers, None);
        assert_eq!(update.threshold, None);
        assert_eq!(update.time_lock_secs, None);
        assert_eq!(update.proposal_lifetime_secs, None);
    }
}
