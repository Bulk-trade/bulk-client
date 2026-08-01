use crate::transaction::actions::Action;
use crate::transaction::TransactionSigner;
use serde::ser::{SerializeSeq, SerializeTuple};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::fmt::Debug;
use std::str::FromStr;

/// Stable signature-domain registry. Zero is deliberately unassigned so a
/// missing or legacy domain cannot silently select a live network.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SignatureDomain {
    Mainnet = 1,
    Testnet = 2,
    Devnet = 3,
}

impl SignatureDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
            Self::Devnet => "devnet",
        }
    }
}

impl std::fmt::Display for SignatureDomain {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SignatureDomain {
    type Err = eyre::Report;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "mainnet" | "Mainnet" => Ok(Self::Mainnet),
            "testnet" | "Testnet" => Ok(Self::Testnet),
            "devnet" | "Devnet" => Ok(Self::Devnet),
            _ => Err(eyre::eyre!(
                "invalid signature domain `{value}`; expected mainnet, testnet, or devnet"
            )),
        }
    }
}

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
    pub(crate) fn raw_signable_bytes(
        signature_domain: SignatureDomain,
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
    ) -> eyre::Result<Vec<u8>> {
        let mut serialized = bincode::serialize(&RawSignableActions(actions))?;
        serialized.extend_from_slice(&nonce.to_le_bytes());
        serialized.extend_from_slice(account.as_ref());
        serialized.push(signature_domain as u8);
        Ok(serialized)
    }

    /// Sign transaction
    /// - NOTE: nonce and account must be filled appropriately before can sign the tx
    ///
    /// # Arguments
    /// - `signer`: tx signer
    pub fn sign(
        &mut self,
        signer: &TransactionSigner,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<()> {
        use crate::transaction::signer::TxSignatureMode;

        match signer.tx_signature_mode() {
            TxSignatureMode::Offchain => {
                let clear_text = crate::transaction::clear_sign::canonical_message(
                    signature_domain,
                    self.account,
                    self.nonce,
                    &self.actions,
                )?;
                self.signature = signer.sign_transaction_clear(&clear_text, signature_domain)?;
            }
            TxSignatureMode::Raw => {
                self.signature = signer.sign_transaction_bytes(
                    Self::raw_signable_bytes(
                        signature_domain,
                        self.account,
                        self.nonce,
                        self.actions.as_slice(),
                    )?
                    .as_slice(),
                    signature_domain,
                )?;
            }
        }
        self.signer = signer.public_key();
        Ok(())
    }

    /// Determine if tx was properly signed
    pub fn verify(&self, signature_domain: SignatureDomain) -> eyre::Result<bool> {
        Ok(self.signature.verify(
            &self.signer.to_bytes(),
            Self::raw_signable_bytes(
                signature_domain,
                self.account,
                self.nonce,
                self.actions.as_slice(),
            )?
            .as_slice(),
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
        let mut tuple =
            serializer.serialize_tuple(5 + usize::from(self.0.builder_code.is_some()))?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&RawSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        if self.0.builder_code.is_some() {
            tuple.serialize_element(&self.0.builder_code)?;
        }
        tuple.end()
    }
}

struct RawSignableLimitOrder<'a>(&'a crate::msgs::LimitOrder);

impl Serialize for RawSignableLimitOrder<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple =
            serializer.serialize_tuple(7 + usize::from(self.0.builder_code.is_some()))?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&RawSafeF64(self.0.price))?;
        tuple.serialize_element(&RawSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.tif)?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        if self.0.builder_code.is_some() {
            tuple.serialize_element(&self.0.builder_code)?;
        }
        tuple.end()
    }
}

struct RawSignableTrigger<'a>(&'a crate::msgs::conditional::Trigger);

impl Serialize for RawSignableTrigger<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(5)?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_above)?;
        tuple.serialize_element(&RawSafeF64(self.0.threshold))?;
        tuple.serialize_element(&RawSignableActions(&self.0.actions))?;
        tuple.end()
    }
}

struct RawSignableOnFill<'a>(&'a crate::msgs::conditional::OnFill);

impl Serialize for RawSignableOnFill<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !matches!(
            self.0.trigger.as_ref(),
            Action::MarketOrder(_) | Action::LimitOrder(_)
        ) {
            return Err(serde::ser::Error::custom(
                "on-fill trigger must be a market or limit order",
            ));
        }
        let mut tuple = serializer.serialize_tuple(2)?;
        tuple.serialize_element(&RawSignableAction(&self.0.trigger))?;
        tuple.serialize_element(&RawSignableActions(&self.0.actions))?;
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
            Action::Trigger(action) => serializer.serialize_newtype_variant(
                "Action",
                8,
                "Trigger",
                &RawSignableTrigger(action),
            ),
            Action::Trailing(action) => {
                serializer.serialize_newtype_variant("Action", 9, "Trailing", action)
            }
            Action::OnFill(action) => serializer.serialize_newtype_variant(
                "Action",
                10,
                "OnFill",
                &RawSignableOnFill(action),
            ),
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
                serializer.serialize_newtype_variant("Action", 39, "UpdateRiskConfig", action)
            }
            Action::ApproveCommissionFee(action) => {
                serializer.serialize_newtype_variant("Action", 40, "ApproveCommissionFee", action)
            }
            Action::RevokeCommissionFee(action) => {
                serializer.serialize_newtype_variant("Action", 41, "RevokeCommissionFee", action)
            }
            Action::UpdateLiquidatorConfig(action) => {
                serializer.serialize_newtype_variant("Action", 43, "UpdateLiquidatorConfig", action)
            }
            Action::MarketAdmin(action) => {
                serializer.serialize_newtype_variant("Action", 57, "MarketAdmin", action)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tif::TimeInForce;
    use crate::msgs::conditional::StopOrTP;
    use crate::msgs::liquidator::LiqConfig;
    use crate::msgs::{CancelAll, Faucet, LimitOrder};
    use crate::transaction::ActionMeta;
    use std::sync::Arc;

    #[cfg(any())]
    use crate::msgs::{ApproveCommissionFee, BuilderCode, RevokeCommissionFee};

    /// A stable base58 seed (32-byte all-zeros key) used only in tests.
    const TEST_PRIVATE_KEY1: &str = "1111111111111111111111111111111111111111111";
    const TEST_PRIVATE_KEY2: &str = "9TucdiMw5Sr5uQMhrxzXivuCAdi7qDLTLASqdSfXX6qH";

    fn bytes_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn trigger_signing_matches_current_sdk_recursive_vector() {
        let action: Action = serde_json::from_value(serde_json::json!({
            "trig": {
                "c": "BTC-USD",
                "d": true,
                "tr": 100000.0,
                "actions": [
                    {"m": {
                        "c": "BTC-USD", "b": true, "sz": 1.25,
                        "r": false, "i": true
                    }},
                    {"l": {
                        "c": "ETH-USD", "b": false, "px": 2500.5, "sz": 2.0,
                        "tif": "ALO", "r": true, "i": false,
                        "builderCode": {
                            "to": "11111111111111111111111111111111",
                            "fee": 5
                        }
                    }}
                ]
            }
        }))
        .expect("canonical trigger JSON");

        assert_eq!(
            bytes_hex(
                &Transaction::raw_signable_bytes(
                    SignatureDomain::Testnet,
                    Pubkey::default(),
                    7,
                    &[action],
                )
                .expect("serialize trigger")
            ),
            "01000000000000000800000007000000000000004254432d5553440100a0724e1809000002000000000000000000000007000000000000004254432d55534401405973070000000000010100000007000000000000004554482d55534400803424383a00000000c2eb0b00000000020000000100010000000000000000000000000000000000000000000000000000000000000000050700000000000000000000000000000000000000000000000000000000000000000000000000000002"
        );
    }

    #[test]
    fn on_fill_signing_matches_current_sdk_inline_trigger_vector() {
        let action: Action = serde_json::from_value(serde_json::json!({
            "of": {
                "trigger": {"l": {
                    "c": "ETH-USD", "b": false, "px": 2500.5, "sz": 2.0,
                    "tif": "ALO", "r": true, "i": false
                }},
                "actions": [
                    {"m": {
                        "c": "BTC-USD", "b": true, "sz": 1.25,
                        "r": false, "i": true
                    }},
                    {"m": {
                        "c": "BTC-USD", "b": true, "sz": 1.25,
                        "r": false, "i": true,
                        "builderCode": {
                            "to": "11111111111111111111111111111111",
                            "fee": 5
                        }
                    }}
                ]
            }
        }))
        .expect("canonical on-fill JSON");

        assert_eq!(
            bytes_hex(
                &Transaction::raw_signable_bytes(
                    SignatureDomain::Testnet,
                    Pubkey::default(),
                    7,
                    &[action],
                )
                .expect("serialize on-fill")
            ),
            "01000000000000000a0000000100000007000000000000004554482d55534400803424383a00000000c2eb0b0000000002000000010002000000000000000000000007000000000000004254432d55534401405973070000000000010000000007000000000000004254432d5553440140597307000000000001010000000000000000000000000000000000000000000000000000000000000000050700000000000000000000000000000000000000000000000000000000000000000000000000000002"
        );
    }

    #[test]
    fn on_fill_signing_rejects_a_non_order_trigger_constructed_in_rust() {
        let on_fill = Action::OnFill(crate::msgs::conditional::OnFill {
            trigger: Box::new(Action::Stop(StopOrTP {
                symbol: Arc::from("BTC-USD"),
                is_above: true,
                size: 1.0,
                threshold: 100.0,
                limit: None,
                iso: false,
                meta: ActionMeta::default(),
            })),
            actions: Vec::new(),
            meta: ActionMeta::default(),
        });

        assert!(
            Transaction::raw_signable_bytes(
                SignatureDomain::Testnet,
                Pubkey::default(),
                7,
                &[on_fill],
            )
            .is_err(),
            "programmatic on-fill values must enforce the same trigger contract as JSON"
        );
    }

    #[test]
    fn signature_domain_registry_is_stable_and_compact() {
        assert_eq!(SignatureDomain::Mainnet as u8, 1);
        assert_eq!(SignatureDomain::Testnet as u8, 2);
        assert_eq!(SignatureDomain::Devnet as u8, 3);
        assert_eq!(std::mem::size_of::<SignatureDomain>(), 1);
    }

    #[test]
    fn update_liquidator_config_uses_sdk_ordinal_43() {
        let signable = Transaction::raw_signable_bytes(
            SignatureDomain::Devnet,
            Pubkey::default(),
            0,
            &[Action::UpdateLiquidatorConfig(LiqConfig::default())],
        )
        .expect("signable bytes");

        assert_eq!(u32::from_le_bytes(signable[8..12].try_into().unwrap()), 43);
    }

    #[test]
    fn market_admin_uses_sdk_ordinal_57() {
        let signable = Transaction::raw_signable_bytes(
            SignatureDomain::Devnet,
            Pubkey::default(),
            0,
            &[Action::MarketAdmin(crate::msgs::MarketAdmin {
                symbol: Arc::from("BTC-USD"),
                action: crate::msgs::MarketAction::Open,
                price: None,
                meta: ActionMeta::default(),
            })],
        )
        .expect("signable bytes");

        assert_eq!(u32::from_le_bytes(signable[8..12].try_into().unwrap()), 57);
    }

    #[test]
    fn transaction_signature_is_bound_to_one_domain() {
        let (mut tx, signer) = make_limit_order_tx();
        tx.sign(&signer, SignatureDomain::Mainnet)
            .expect("mainnet signature");

        assert!(tx
            .verify(SignatureDomain::Mainnet)
            .expect("mainnet verification"));
        assert!(!tx
            .verify(SignatureDomain::Testnet)
            .expect("testnet verification"));
        assert!(!tx
            .verify(SignatureDomain::Devnet)
            .expect("devnet verification"));
        assert!(serde_json::to_value(&tx)
            .expect("transaction JSON")
            .get("signatureDomain")
            .is_none());
    }

    #[test]
    fn signature_domain_changes_only_the_signable_suffix() {
        let (tx, _) = make_limit_order_tx();
        let mainnet = Transaction::raw_signable_bytes(
            SignatureDomain::Mainnet,
            tx.account,
            tx.nonce,
            &tx.actions,
        )
        .expect("mainnet bytes");
        let testnet = Transaction::raw_signable_bytes(
            SignatureDomain::Testnet,
            tx.account,
            tx.nonce,
            &tx.actions,
        )
        .expect("testnet bytes");

        assert_eq!(mainnet.len(), testnet.len());
        assert_eq!(&mainnet[..mainnet.len() - 1], &testnet[..testnet.len() - 1]);
        assert_eq!(mainnet[mainnet.len() - 1], SignatureDomain::Mainnet as u8);
        assert_eq!(testnet[testnet.len() - 1], SignatureDomain::Testnet as u8);
    }

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
            builder_code: None,
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

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        // Signer pubkey must be populated after signing
        assert_eq!(tx.signer, signer.public_key());

        // Signature must not be the default all-zero value
        assert_ne!(tx.signature, Signature::default());

        eprintln!("limit_order signature: {}", tx.signature);

        // Verification must pass
        assert!(
            tx.verify(SignatureDomain::Devnet)
                .expect("verify should not error"),
            "limit order signature verification failed"
        );
    }

    // These SDK parity checks require a sibling bulk-sdk checkout. Keep them out of
    // standalone package builds and publish workflows.
    #[cfg(any())]
    #[test]
    fn limit_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_limit_order_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify(bulk_transaction::SignatureDomain::Devnet)
            .expect("sdk verify should not error"),
            "client-signed limit order must verify with sdk server bytes"
        );
    }

    #[cfg(any())]
    #[test]
    fn commissioned_limit_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_limit_order_tx();
        if let Action::LimitOrder(ref mut order) = tx.actions[0] {
            order.builder_code = Some(BuilderCode {
                to: Pubkey::new_unique(),
                fee: 5,
            });
        }
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify(bulk_transaction::SignatureDomain::Devnet)
            .expect("sdk verify should not error"),
            "client-signed commissioned limit order must verify with sdk server bytes"
        );
    }

    #[test]
    fn limit_order_tx_tampered_price_fails_verify() {
        let (mut tx, signer) = make_limit_order_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        // Tamper with the action payload after signing
        if let Action::LimitOrder(ref mut o) = tx.actions[0] {
            o.price = 1.0;
        }

        let valid = tx
            .verify(SignatureDomain::Devnet)
            .expect("verify should not error");
        assert!(!valid, "tampered limit order should not verify");
    }

    #[cfg(any())]
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
            builder_code: None,
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

    #[cfg(any())]
    #[test]
    fn market_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_market_order_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify(bulk_transaction::SignatureDomain::Devnet)
            .expect("sdk verify should not error"),
            "client-signed market order must verify with sdk server bytes"
        );
    }

    #[cfg(any())]
    #[test]
    fn commissioned_market_order_signature_verifies_after_sdk_json_deserialize() {
        let (mut tx, signer) = make_market_order_tx();
        if let Action::MarketOrder(ref mut order) = tx.actions[0] {
            order.builder_code = Some(BuilderCode {
                to: Pubkey::new_unique(),
                fee: 15,
            });
        }
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify(bulk_transaction::SignatureDomain::Devnet)
            .expect("sdk verify should not error"),
            "client-signed commissioned market order must verify with sdk server bytes"
        );
    }

    #[cfg(any())]
    #[test]
    fn commission_approval_actions_verify_after_sdk_json_deserialize() {
        let signer =
            TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).expect("valid test key");
        let account = signer.public_key();
        let to = Pubkey::new_unique();
        let mut tx = Transaction {
            actions: vec![
                Action::ApproveCommissionFee(ApproveCommissionFee {
                    to,
                    max_fee: 5,
                    meta: ActionMeta {
                        account,
                        nonce: 45,
                        seqno: 0,
                        ..Default::default()
                    },
                }),
                Action::RevokeCommissionFee(RevokeCommissionFee {
                    to,
                    meta: ActionMeta {
                        account,
                        nonce: 45,
                        seqno: 1,
                        ..Default::default()
                    },
                }),
            ],
            nonce: 45,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert!(
            serde_json::from_str::<bulk_transaction::Transaction>(
                serde_json::to_string(&tx)
                    .expect("client transaction should serialize")
                    .as_str()
            )
            .expect("sdk transaction should deserialize")
            .verify(bulk_transaction::SignatureDomain::Devnet)
            .expect("sdk verify should not error"),
            "client-signed commission approval actions must verify with sdk server bytes"
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

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        // Signer pubkey must be populated after signing
        assert_eq!(tx.signer, signer.public_key());

        // Signature must not be the default all-zero value
        assert_ne!(tx.signature, Signature::default());

        eprintln!("cancel_all signature: {}", tx.signature);

        // Verification must pass
        assert!(
            tx.verify(SignatureDomain::Devnet)
                .expect("verify should not error"),
            "cancel_all signature verification failed"
        );
    }

    #[test]
    fn cancel_all_tx_tampered_symbols_fails_verify() {
        let (mut tx, signer) = make_cancel_all_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        // Tamper with the symbol list after signing
        if let Action::CancelAll(ref mut c) = tx.actions[0] {
            c.symbols.push("SOL-PERP".to_string());
        }

        let valid = tx
            .verify(SignatureDomain::Devnet)
            .expect("verify should not error");
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

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!(
            "faucet signature: {}, account: {}",
            tx.signature,
            signer.public_key()
        );

        assert!(
            tx.verify(SignatureDomain::Devnet)
                .expect("verify should not error"),
            "faucet signature verification failed"
        );
    }

    #[test]
    fn faucet_tx_tampered_user_fails_verify() {
        let (mut tx, signer) = make_faucet_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        // Tamper with the user pubkey after signing
        if let Action::Faucet(ref mut f) = tx.actions[0] {
            f.user = Pubkey::new_unique();
        }

        let valid = tx
            .verify(SignatureDomain::Devnet)
            .expect("verify should not error");
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
            iso: false,
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
            iso: false,
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

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!("take_profit1 signature: {}", tx.signature);

        assert!(
            tx.verify(SignatureDomain::Devnet)
                .expect("verify should not error"),
            "take_profit signature verification failed"
        );
    }

    #[test]
    fn take_profit_tx_sign_and_verify2() {
        let (mut tx, signer) = make_take_profit_tx2();

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        assert_eq!(tx.signer, signer.public_key());
        assert_ne!(tx.signature, Signature::default());

        eprintln!("take_profit2 signature: {}", tx.signature);

        assert!(
            tx.verify(SignatureDomain::Devnet)
                .expect("verify should not error"),
            "take_profit signature verification failed"
        );
    }

    #[test]
    fn take_profit_tx_tampered_threshold_fails_verify() {
        let (mut tx, signer) = make_take_profit_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        if let Action::TakeProfit(ref mut tp) = tx.actions[0] {
            tp.threshold = 80_000.0;
        }

        let valid = tx
            .verify(SignatureDomain::Devnet)
            .expect("verify should not error");
        assert!(!valid, "tampered take_profit threshold should not verify");
    }

    #[test]
    fn take_profit_tx_tampered_size_fails_verify() {
        let (mut tx, signer) = make_take_profit_tx();
        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign should succeed");

        if let Action::TakeProfit(ref mut tp) = tx.actions[0] {
            tp.size = 1.0;
        }

        let valid = tx
            .verify(SignatureDomain::Devnet)
            .expect("verify should not error");
        assert!(!valid, "tampered take_profit size should not verify");
    }
}
