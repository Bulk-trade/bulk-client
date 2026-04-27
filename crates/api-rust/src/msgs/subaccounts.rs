use std::sync::Arc;
use serde::{Deserialize, Serialize};
use solana_keypair::Pubkey;
use crate::transaction::ActionMeta;


// ─────────────────────────────────────────────────────────────────────────────
// Type of transfer
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferKind {
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "external")]
    External,
}

impl Default for TransferKind {
    fn default() -> Self {
        Self::Internal
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Actions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubAccount {
    /// sub-account name
    pub name: Arc<str>,
    /// which asset in margin to transfer to sub-account
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_symbol: Option<Arc<str>>,
    /// amount of margin to transfer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_amount: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSubAccount {
    /// sub-account to be removed
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub to_remove: Pubkey,

    #[serde(skip)]
    pub meta: ActionMeta,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    /// transfer type
    #[serde(rename = "k", default)]
    pub kind: TransferKind,

    /// pubkey of account to transfer from
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub from: Pubkey,
    /// pubkey of account to transfer to
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub to: Pubkey,

    /// which asset in margin to transfer
    pub margin_symbol: Arc<str>,
    /// amount of instrument to transfer
    pub margin_amount: f64,

    #[serde(skip)]
    pub meta: ActionMeta,
}
