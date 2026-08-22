use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::sync::Arc;

/// Settles validator reward shares for an epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewardSettlement {
    pub epoch: u32,
    pub weights: Vec<(Pubkey, u32)>,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Credits a user after an observed Solana vault deposit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deposit {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "u")]
    pub user: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "v")]
    pub vault: Pubkey,
    #[serde(rename = "a")]
    pub amount: u64,
    #[serde(with = "crate::msgs::serde_signature", rename = "ss")]
    pub solana_signature: Signature,
    #[serde(rename = "ii")]
    pub instruction_index: u16,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Requests a withdrawal from a Bulk vault.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Withdraw {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "u")]
    pub user: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "v")]
    pub vault: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "rta")]
    pub recipient_token_account: Pubkey,
    #[serde(rename = "a")]
    pub amount: u64,
    #[serde(with = "crate::msgs::serde_hash", rename = "b")]
    pub blockhash: Hash,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Confirms a submitted withdrawal on Solana.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawConfirmation {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "u")]
    pub user_token_account: Pubkey,
    #[serde(with = "crate::msgs::serde_hash", rename = "b")]
    pub hash: Hash,
    #[serde(with = "crate::msgs::serde_signature", rename = "s")]
    pub signature: Signature,
    #[serde(rename = "ii")]
    pub instruction_index: u16,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Records a withdrawal submitted to Solana.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawSubmitted {
    #[serde(with = "crate::msgs::serde_hash", rename = "b")]
    pub hash: Hash,
    #[serde(with = "crate::msgs::serde_signature", rename = "s")]
    pub signature: Signature,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "r")]
    pub recipient_token_account: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "v")]
    pub vault: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "va")]
    pub vault_token_account: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub mint: Pubkey,
    #[serde(rename = "a")]
    pub amount: u64,
    #[serde(with = "crate::msgs::serde_hash", rename = "bh")]
    pub blockhash: Hash,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Records a terminal withdrawal failure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WithdrawFailed {
    #[serde(with = "crate::msgs::serde_hash", rename = "b")]
    pub hash: Hash,
    #[serde(rename = "r")]
    pub reason: String,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Publishes a FROST nonce commitment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NonceCommitment {
    pub signer: Pubkey,
    pub hiding: [u8; 32],
    pub binding: [u8; 32],
    #[serde(with = "crate::msgs::serde_hash")]
    pub session_id: Hash,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Publishes a FROST partial signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialSignature {
    pub signer: Pubkey,
    #[serde(with = "crate::msgs::serde_hash")]
    pub session_id: Hash,
    pub share: [u8; 32],
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Publishes the first distributed-key-generation round package.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DkgRound1 {
    pub signer: Pubkey,
    pub epoch: u64,
    pub package: Vec<u8>,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Marks distributed-key-generation staging complete.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DkgFinished {
    pub signer: Pubkey,
    pub epoch: u64,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Registers a Solana vault and its token account.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InitializeVault {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "v")]
    pub vault: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "m")]
    pub mint: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "t")]
    pub token_account: Pubkey,
    #[serde(with = "crate::msgs::serde_signature", rename = "ss")]
    pub solana_signature: Signature,
    #[serde(rename = "ii")]
    pub instruction_index: u16,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Updates the Solana FROST group configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateFrostGroup {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "st")]
    pub state: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "fg")]
    pub frost_group_key: Pubkey,
    #[serde(with = "crate::msgs::serde_signature", rename = "ss")]
    pub solana_signature: Signature,
    #[serde(rename = "ii")]
    pub instruction_index: u16,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Anchors a Solana blockhash at a slot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolanaBlockAnchor {
    pub slot: u64,
    #[serde(with = "crate::msgs::serde_hash", rename = "bh")]
    pub blockhash: Hash,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// One-time migration credit from an on-chain pre-deposit PDA snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreDepositCredit {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "u")]
    pub user: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "v")]
    pub vault: Pubkey,
    #[serde(rename = "a")]
    pub amount: u64,
    #[serde(rename = "ms")]
    pub migration_slot: u64,
    #[serde(with = "crate::msgs::serde_pubkey", rename = "pda")]
    pub pre_deposit_pda: Pubkey,
    #[serde(rename = "ei")]
    pub entry_index: u8,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Configures a market-specific maker rebate tier override.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMakerRebateTier {
    pub instrument: Arc<str>,
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub maker: Pubkey,
    pub minimum_tier: Option<u8>,
    pub expires_slot: Option<u64>,
    #[serde(skip)]
    pub meta: ActionMeta,
}

/// Optional account funding policy changes applied atomically by protocol administration.
///
/// `None` preserves the current value. Monetary values are denominated in USD.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountPolicy {
    /// Fixed bridge-withdraw fee credited to the protocol fee account.
    pub withdraw_fee_usd: Option<f64>,
    /// Minimum amount that may be requested for a bridge withdrawal.
    pub min_withdraw_usd: Option<f64>,
    /// Minimum amount permitted for an external account transfer.
    pub min_external_transfer_usd: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}
