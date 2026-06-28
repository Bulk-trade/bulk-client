use crate::transaction::actions::Action;
use crate::transaction::TransactionSigner;
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::fmt::Debug;

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
    fn raw_signable_bytes(
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
    ) -> eyre::Result<Vec<u8>> {
        let mut serialized = bincode::serialize(&RawSignableActions(actions))?;
        serialized.extend_from_slice(&nonce.to_le_bytes());
        serialized.extend_from_slice(account.as_ref());
        Ok(serialized)
    }

    /// Sign transaction
    /// - NOTE: nonce and account must be filled appropriately before can sign the tx
    ///
    /// # Arguments
    /// - `signer`: tx signer
    pub fn sign(&mut self, signer: &TransactionSigner) -> eyre::Result<()> {
        use crate::transaction::signer::TxSignatureMode;

        match signer.tx_signature_mode() {
            TxSignatureMode::Offchain => {
                let clear_text = crate::transaction::clear_sign::canonical_message(
                    self.account,
                    self.nonce,
                    &self.actions,
                )?;
                self.signature = signer.sign_transaction_clear(&clear_text)?;
            }
            TxSignatureMode::Raw => {
                self.signature = signer.sign_transaction_bytes(
                    Self::raw_signable_bytes(self.account, self.nonce, self.actions.as_slice())?
                        .as_slice(),
                )?;
            }
        }
        self.signer = signer.public_key();
        Ok(())
    }

    /// Determine if tx was properly signed
    pub fn verify(&self) -> eyre::Result<bool> {
        Ok(self.signature.verify(
            &self.signer.to_bytes(),
            Self::raw_signable_bytes(self.account, self.nonce, self.actions.as_slice())?.as_slice(),
        ))
    }
}

struct RawSafeF64(f64);

impl Serialize for RawSafeF64 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::msgs::fixed_point::serialize(&self.0, serializer)
    }
}

struct RawSignableMarketOrder<'a>(&'a crate::msgs::MarketOrder);

impl Serialize for RawSignableMarketOrder<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(5 + usize::from(self.0.commission.is_some()))?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&RawSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        if let Some(commission) = &self.0.commission {
            tuple.serialize_element(commission)?;
        }
        tuple.end()
    }
}

struct RawSignableLimitOrder<'a>(&'a crate::msgs::LimitOrder);

impl Serialize for RawSignableLimitOrder<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(7 + usize::from(self.0.commission.is_some()))?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&RawSafeF64(self.0.price))?;
        tuple.serialize_element(&RawSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.tif)?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        if let Some(commission) = &self.0.commission {
            tuple.serialize_element(commission)?;
        }
        tuple.end()
    }
}

struct RawSignableActions<'a>(&'a [Action]);

impl Serialize for RawSignableActions<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for action in self.0 {
            seq.serialize_element(&RawSignableAction(action))?;
        }
        seq.end()
    }
}

struct RawSignableAction<'a>(&'a Action);

impl Serialize for RawSignableAction<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Action::MarketOrder(action) => serializer.serialize_newtype_variant(
                "Action",
                0,
                "MarketOrder",
                &RawSignableMarketOrder(action),
            ),
            Action::LimitOrder(action) => serializer.serialize_newtype_variant(
                "Action",
                1,
                "LimitOrder",
                &RawSignableLimitOrder(action),
            ),
            Action::ModifyOrder(action) => {
                serializer.serialize_newtype_variant("Action", 2, "ModifyOrder", action)
            }
            Action::Cancel(action) => {
                serializer.serialize_newtype_variant("Action", 3, "Cancel", action)
            }
            Action::CancelAll(action) => {
                serializer.serialize_newtype_variant("Action", 4, "CancelAll", action)
            }
            Action::Stop(action) => {
                serializer.serialize_newtype_variant("Action", 5, "Stop", action)
            }
            Action::TakeProfit(action) => {
                serializer.serialize_newtype_variant("Action", 6, "TakeProfit", action)
            }
            Action::Range(action) => {
                serializer.serialize_newtype_variant("Action", 7, "Range", action)
            }
            Action::Trigger(action) => {
                serializer.serialize_newtype_variant("Action", 8, "Trigger", action)
            }
            Action::Trailing(action) => {
                serializer.serialize_newtype_variant("Action", 9, "Trailing", action)
            }
            Action::OnFill(action) => {
                serializer.serialize_newtype_variant("Action", 10, "OnFill", action)
            }
            Action::Price(action) => {
                serializer.serialize_newtype_variant("Action", 11, "Price", action)
            }
            Action::Corrs(action) => {
                serializer.serialize_newtype_variant("Action", 12, "Corrs", action)
            }
            Action::PythOracle(action) => {
                serializer.serialize_newtype_variant("Action", 13, "PythOracle", action)
            }
            Action::Beacon(action) => {
                serializer.serialize_newtype_variant("Action", 14, "Beacon", action)
            }
            Action::Join(action) => {
                serializer.serialize_newtype_variant("Action", 15, "Join", action)
            }
            Action::Faucet(action) => {
                serializer.serialize_newtype_variant("Action", 16, "Faucet", action)
            }
            Action::AgentWalletCreation(action) => {
                serializer.serialize_newtype_variant("Action", 17, "AgentWalletCreation", action)
            }
            Action::UpdateUserSettings(action) => {
                serializer.serialize_newtype_variant("Action", 18, "UpdateUserSettings", action)
            }
            Action::WhitelistFaucet(action) => {
                serializer.serialize_newtype_variant("Action", 19, "WhitelistFaucet", action)
            }
            Action::AddMarket(action) => {
                serializer.serialize_newtype_variant("Action", 20, "AddMarket", action)
            }
            Action::ConfigFairPrice(action) => {
                serializer.serialize_newtype_variant("Action", 21, "ConfigFairPrice", action)
            }
            Action::ConfigVolatility(action) => {
                serializer.serialize_newtype_variant("Action", 22, "ConfigVolatility", action)
            }
            Action::ConfigSecurity(action) => {
                serializer.serialize_newtype_variant("Action", 23, "ConfigSecurity", action)
            }
            Action::ConfigRegime(action) => {
                serializer.serialize_newtype_variant("Action", 24, "ConfigRegime", action)
            }
            Action::ConfigRisk(action) => {
                serializer.serialize_newtype_variant("Action", 25, "ConfigRisk", action)
            }
            Action::ConfigFeePolicy(action) => {
                serializer.serialize_newtype_variant("Action", 26, "ConfigFeePolicy", action)
            }
            Action::CreateSubAccount(action) => {
                serializer.serialize_newtype_variant("Action", 27, "CreateSubAccount", action)
            }
            Action::RemoveSubAccount(action) => {
                serializer.serialize_newtype_variant("Action", 28, "RemoveSubAccount", action)
            }
            Action::Transfer(action) => {
                serializer.serialize_newtype_variant("Action", 29, "Transfer", action)
            }
            Action::CreateMultisig(action) => {
                serializer.serialize_newtype_variant("Action", 30, "CreateMultisig", action)
            }
            Action::MultisigPropose(action) => {
                serializer.serialize_newtype_variant("Action", 31, "MultisigPropose", action)
            }
            Action::MultisigApprove(action) => {
                serializer.serialize_newtype_variant("Action", 32, "MultisigApprove", action)
            }
            Action::MultisigReject(action) => {
                serializer.serialize_newtype_variant("Action", 33, "MultisigReject", action)
            }
            Action::MultisigCancel(action) => {
                serializer.serialize_newtype_variant("Action", 34, "MultisigCancel", action)
            }
            Action::MultisigExecute(action) => {
                serializer.serialize_newtype_variant("Action", 35, "MultisigExecute", action)
            }
            Action::UpdateMultisigPolicy(action) => {
                serializer.serialize_newtype_variant("Action", 36, "UpdateMultisigPolicy", action)
            }
            Action::RenameSubAccount(action) => {
                serializer.serialize_newtype_variant("Action", 37, "RenameSubAccount", action)
            }
            Action::UpdateValidatorSet(action) => {
                serializer.serialize_newtype_variant("Action", 38, "UpdateValidatorSet", action)
            }
            Action::UpdateRiskConfig(action) => {
                serializer.serialize_newtype_variant("Action", 40, "UpdateRiskConfig", action)
            }
            Action::UpdateLiquidatorConfig(action) => {
                serializer.serialize_newtype_variant("Action", 41, "UpdateLiquidatorConfig", action)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tif::TimeInForce;
    use crate::msgs::conditional::StopOrTP;
    use crate::msgs::{CancelAll, Faucet, LimitOrder};
    use crate::transaction::ActionMeta;
    use std::sync::Arc;

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
            commission: None,
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
    fn limit_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_limit_order_tx();
        tx.sign(&signer).expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify()
            .expect("sdk verify should not error"),
            "client-signed limit order must verify with sdk server bytes"
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

    fn make_market_order_tx() -> (Transaction, TransactionSigner) {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");

        let account = signer.public_key();

        let action = Action::MarketOrder(crate::msgs::MarketOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: false,
            size: 0.25,
            reduce_only: false,
            iso: false,
            commission: None,
            meta: ActionMeta {
                account,
                nonce: 43,
                seqno: 0,
                ..Default::default()
            },
        });

        let tx = Transaction {
            actions: vec![action],
            nonce: 43,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn market_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_market_order_tx();
        tx.sign(&signer).expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify()
            .expect("sdk verify should not error"),
            "client-signed market order must verify with sdk server bytes"
        );
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
            meta: Default::default(),
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

        eprintln!(
            "faucet signature: {}, account: {}",
            tx.signature,
            signer.public_key()
        );

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
