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

#[allow(unused)]
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

    /// Determine if tx was properly signed
    pub fn verify(&self) -> eyre::Result<bool> {
        // get serialized form: actions nonce + account
        let mut serialized = bincode::serialize(&self.actions)?;
        serialized.extend_from_slice(&self.nonce.to_le_bytes());
        serialized.extend_from_slice(self.account.as_ref());

        Ok(self.signature.verify(&self.signer.to_bytes(), &serialized))
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::common::tif::TimeInForce;
    use crate::msgs::{CancelAll, Faucet, LimitOrder};
    use crate::msgs::conditional::StopOrTP;
    use crate::transaction::ActionMeta;

    /// A stable base58 seed (32-byte all-zeros key) used only in tests.
    const TEST_PRIVATE_KEY1: &str = "1111111111111111111111111111111111111111111";
    const TEST_PRIVATE_KEY2: &str = "9TucdiMw5Sr5uQMhrxzXivuCAdi7qDLTLASqdSfXX6qH";

    // -----------------------------------------------------------------------
    // LimitOrder
    // -----------------------------------------------------------------------

    fn make_limit_order_tx() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");

        let account = signer.public_key();

        let action = Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 65_000.0,
            size: 0.5,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            meta: ActionMeta {
                account,
                nonce: 42,
                seqno: 0,
                ..Default::default()
            },
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 42,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn limit_order_tx_sign_and_verify() {
        let (mut tx, signer) = make_limit_order_tx();

        tx.sign(&signer).expect("sign should succeed");

        // Signer pubkey must be populated after signing
        assert_eq!(tx.signer, signer.public_key());

        // Signature must not be the default all-zero value
        assert_ne!(tx.signature, Signature::default());

        eprintln!("limit_order signature: {}", tx.signature);

        // Verification must pass
        assert!(
            tx.verify().expect("verify should not error"),
            "limit order signature verification failed"
        );
    }

    #[test]
    fn limit_order_tx_tampered_price_fails_verify() {
        let (mut tx, signer) = make_limit_order_tx();
        tx.sign(&signer).expect("sign should succeed");

        // Tamper with the action payload after signing
        if let Action::LimitOrder(ref mut o) = tx.actions[0] {
            o.price = 1.0;
        }

        let valid = tx.verify().expect("verify should not error");
        assert!(!valid, "tampered limit order should not verify");
    }

    // -----------------------------------------------------------------------
    // CancelAll
    // -----------------------------------------------------------------------

    fn make_cancel_all_tx() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");

        let account = signer.public_key();

        let action = Action::CancelAll(CancelAll {
            symbols: vec!["BTC-USD".to_string()],
            meta: ActionMeta {
                account,
                nonce: 42,
                seqno: 0,
                ..Default::default()
            },
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 42,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn cancel_all_tx_sign_and_verify() {
        let (mut tx, signer) = make_cancel_all_tx();

        tx.sign(&signer).expect("sign should succeed");

        // Signer pubkey must be populated after signing
        assert_eq!(tx.signer, signer.public_key());

        // Signature must not be the default all-zero value
        assert_ne!(tx.signature, Signature::default());

        eprintln!("cancel_all signature: {}", tx.signature);

        // Verification must pass
        assert!(
            tx.verify().expect("verify should not error"),
            "cancel_all signature verification failed"
        );
    }

    #[test]
    fn cancel_all_tx_tampered_symbols_fails_verify() {
        let (mut tx, signer) = make_cancel_all_tx();
        tx.sign(&signer).expect("sign should succeed");

        // Tamper with the symbol list after signing
        if let Action::CancelAll(ref mut c) = tx.actions[0] {
            c.symbols.push("SOL-PERP".to_string());
        }

        let valid = tx.verify().expect("verify should not error");
        assert!(!valid, "tampered cancel_all should not verify");
    }

    // -----------------------------------------------------------------------
    // Faucet (no amount)
    // -----------------------------------------------------------------------

    fn make_faucet_tx() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY2).expect("valid test key");

        let account = signer.public_key();

        let action = Action::Faucet(Faucet {
            user: account,
            amount: None,
            meta: Default::default()
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 1776678783594,
            account,
            signer: signer.public_key(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn faucet_tx_sign_and_verify() {
        let (mut tx, signer) = make_faucet_tx();

        tx.sign(&signer).expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!("faucet signature: {}, account: {}", tx.signature, signer.public_key());

        assert!(
            tx.verify().expect("verify should not error"),
            "faucet signature verification failed"
        );
    }

    #[test]
    fn faucet_tx_tampered_user_fails_verify() {
        let (mut tx, signer) = make_faucet_tx();
        tx.sign(&signer).expect("sign should succeed");

        // Tamper with the user pubkey after signing
        if let Action::Faucet(ref mut f) = tx.actions[0] {
            f.user = Pubkey::new_unique();
        }

        let valid = tx.verify().expect("verify should not error");
        assert!(!valid, "tampered faucet user should not verify");
    }

    // -----------------------------------------------------------------------
    // TakeProfit (market trigger, no limit)
    // -----------------------------------------------------------------------

    fn make_take_profit_tx() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");

        let account = signer.public_key();

        let action = Action::TakeProfit(StopOrTP {
            symbol: Arc::from("BTC-USD"),
            is_above: true, // triggers when price rises above threshold
            size: 2.0,
            threshold: 60_000.0,
            limit: Some(60_010.0),
            meta: Default::default(),
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 42,
            account,
            signer: signer.public_key(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    fn make_take_profit_tx2() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");

        let account = signer.public_key();

        let action = Action::TakeProfit(StopOrTP {
            symbol: Arc::from("BTC-USD"),
            is_above: true, // triggers when price rises above threshold
            size: 2.0,
            threshold: 60_000.0,
            limit: None,
            meta: Default::default(),
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 42,
            account,
            signer: signer.public_key(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn take_profit_tx_sign_and_verify1() {
        let (mut tx, signer) = make_take_profit_tx();

        tx.sign(&signer).expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!("take_profit1 signature: {}", tx.signature);

        assert!(
            tx.verify().expect("verify should not error"),
            "take_profit signature verification failed"
        );
    }

    #[test]
    fn take_profit_tx_sign_and_verify2() {
        let (mut tx, signer) = make_take_profit_tx2();

        tx.sign(&signer).expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!("take_profit2 signature: {}", tx.signature);

        assert!(
            tx.verify().expect("verify should not error"),
            "take_profit signature verification failed"
        );
    }

    #[test]
    fn take_profit_tx_tampered_threshold_fails_verify() {
        let (mut tx, signer) = make_take_profit_tx();
        tx.sign(&signer).expect("sign should succeed");

        if let Action::TakeProfit(ref mut tp) = tx.actions[0] {
            tp.threshold = 80_000.0;
        }

        let valid = tx.verify().expect("verify should not error");
        assert!(!valid, "tampered take_profit threshold should not verify");
    }

    #[test]
    fn take_profit_tx_tampered_size_fails_verify() {
        let (mut tx, signer) = make_take_profit_tx();
        tx.sign(&signer).expect("sign should succeed");

        if let Action::TakeProfit(ref mut tp) = tx.actions[0] {
            tp.size = 1.0;
        }

        let valid = tx.verify().expect("verify should not error");
        assert!(!valid, "tampered take_profit size should not verify");
    }
}
