use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use crate::transaction::actions::Action;
use crate::transaction::TransactionSigner;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    // list of actions in tx
    pub actions: Vec<Action>,
    // tx nonce
    pub nonce: u64,
    // account tx to be applied to
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub account: Pubkey,
    // tx signer (which may be different from account if agent)
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub signer: Pubkey,
    // signature
    #[serde(with = "crate::msgs::serde_signature")]
    pub signature: Signature,
}

impl Transaction {
    /// Sign transaction
    /// - NOTE: nonce and account must be filled appropriately before can sign the tx
    ///
    /// # Arguments
    /// - `signer`: tx signer
    pub fn sign(&mut self, signer: &TransactionSigner) -> eyre::Result<()> {
        // get serialized form: actions nonce + account
        let mut serialized = bincode::serialize(&self.actions)?;
        serialized.extend_from_slice(&self.nonce.to_le_bytes());
        serialized.extend_from_slice(self.account.as_ref());

        // compute signature
        self.signature = signer.sign_bytes(&serialized);
        self.signer = signer.public_key();
        Ok(())
    }
}
