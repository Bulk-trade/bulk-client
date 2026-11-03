use crate::transaction::actions::Action;
use crate::transaction::{TransactionSigner, TransactionVersion};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use std::fmt::Debug;
use std::str::FromStr;

fn validate_nested_actions(action: &Action) -> eyre::Result<()> {
    match action {
        Action::Trigger(trigger) => {
            for child in &trigger.actions {
                validate_nested_actions(child)?;
            }
        }
        Action::OnFill(on_fill) => {
            if !matches!(
                on_fill.trigger.as_ref(),
                Action::MarketOrder(_) | Action::LimitOrder(_)
            ) {
                eyre::bail!("on-fill trigger must be a market or limit order");
            }
            validate_nested_actions(&on_fill.trigger)?;
            for child in &on_fill.actions {
                validate_nested_actions(child)?;
            }
        }
        Action::MultisigPropose(proposal) => {
            for child in &proposal.actions {
                validate_nested_actions(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum SignatureDomain {
    Mainnet = 1,
    Testnet = 2,
    Devnet = 3,
}

impl SignatureDomain {
    /// Returns the lowercase network name.
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
    /// Schema used for wire encoding and signatures.
    pub version: TransactionVersion,
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
    // ───── Public API Contract ───────────────────────────────────────────────────────────────────

    /// Signs the transaction using the signer's configured mode.
    ///
    /// * Encode a clear text or raw payload for the signer.
    /// * Sign it and record the signing public key.
    ///
    /// # Arguments
    /// * `signer` - Key and mode used to sign the transaction.
    /// * `signature_domain` - Network domain bound to the signature.
    ///
    /// # Returns
    /// An error if the payload cannot be encoded or signed.
    pub fn sign(
        &mut self,
        signer: &TransactionSigner,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<()> {
        use crate::transaction::signer::TxSignatureMode;

        match signer.tx_signature_mode() {
            TxSignatureMode::Offchain => {
                let clear_text = crate::transaction::ClearSignMessage::canonical_message(
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
                        self.version,
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

    /// Verifies the transaction's raw signature.
    ///
    /// # Arguments
    /// * `signature_domain` - Network domain to verify against.
    ///
    /// # Returns
    /// Whether the signature is valid, or a payload encoding error.
    pub fn verify(&self, signature_domain: SignatureDomain) -> eyre::Result<bool> {
        Ok(self.signature.verify(
            &self.signer.to_bytes(),
            Self::raw_signable_bytes(
                self.version,
                signature_domain,
                self.account,
                self.nonce,
                self.actions.as_slice(),
            )?
            .as_slice(),
        ))
    }

    // ───── Signing Internals ─────────────────────────────────────────────────────────────────────

    /// Encodes the stable transaction payload for raw signing.
    ///
    /// * Prefix the numeric V12 version and encode the fixed action schema.
    /// * Serialize the actions and append the transaction context.
    ///
    /// # Arguments
    /// * `version` - Explicit transaction schema (V12).
    /// * `signature_domain` - Network domain appended to the payload.
    /// * `account` - Account bound to the signature.
    /// * `nonce` - Transaction nonce bound to the signature.
    /// * `actions` - Actions encoded using the fixed V12 layout.
    ///
    /// # Returns
    /// The signable bytes, or an action encoding error.
    pub(crate) fn raw_signable_bytes(
        version: TransactionVersion,
        signature_domain: SignatureDomain,
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
    ) -> eyre::Result<Vec<u8>> {
        for action in actions {
            validate_nested_actions(action)?;
        }
        let mut serialized = Vec::new();
        serialized.extend_from_slice(&version.number().to_le_bytes());
        bincode::serialize_into(&mut serialized, actions)?;
        serialized.extend_from_slice(&nonce.to_le_bytes());
        serialized.extend_from_slice(account.as_ref());
        serialized.push(signature_domain as u8);
        Ok(serialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tif::TimeInForce;
    use crate::msgs::conditional::StopOrTP;
    use crate::msgs::liquidator::{LiqConfig, LiqConfigByInstrument};
    use crate::msgs::{CancelAll, Faucet, LimitOrder, MultisigPropose};
    use crate::transaction::ActionMeta;
    use std::sync::Arc;

    #[cfg(any())]
    use crate::msgs::{ApproveCommissionFee, BuilderCode, RevokeCommissionFee};

    /// A stable base58 seed (32-byte all-zeros key) used only in tests.
    const TEST_PRIVATE_KEY1: &str = "11111111111111111111111111111111";
    const TEST_PRIVATE_KEY2: &str = "9TucdiMw5Sr5uQMhrxzXivuCAdi7qDLTLASqdSfXX6qH";

    fn bytes_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn leaf_conditionals_use_v12_with_or_without_builder_code() {
        use serde_json::json;

        let account = Pubkey::new_from_array([1; 32]);
        let cases = [
            json!({"st":{"c":"BTC-USD","d":true,"sz":1.0,"tr":100.0,"lim":null,"i":false}}),
            json!({"tp":{"c":"BTC-USD","d":false,"sz":1.0,"tr":100.0,"lim":null,"i":false}}),
            json!({"rng":{"c":"BTC-USD","d":true,"sz":1.0,"pmin":90.0,"pmax":110.0,"lmin":null,"lmax":null,"i":false}}),
            json!({"trl":{"c":"BTC-USD","b":true,"sz":1.0,"trb":100,"stb":10,"lim":null,"i":false}}),
        ];
        for mut value in cases {
            let action: Action = serde_json::from_value(value.clone()).unwrap();
            let without_builder = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                account,
                42,
                &[action],
            )
            .unwrap();
            assert!(without_builder.starts_with(&12u32.to_le_bytes()));

            let kind = value.as_object().unwrap().keys().next().unwrap().clone();
            value[&kind]["builderCode"] =
                json!({"to":Pubkey::new_from_array([7;32]).to_string(),"fee":5});
            let action: Action = serde_json::from_value(value).unwrap();
            let with_builder = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                account,
                42,
                &[action],
            )
            .unwrap();
            assert!(with_builder.starts_with(&12u32.to_le_bytes()));
            assert_ne!(without_builder, with_builder);
        }
    }

    #[test]
    fn v12_conditionals_preserve_multi_action_boundaries() {
        let account = Pubkey::new_from_array([1; 32]);
        let actions: Vec<Action> = serde_json::from_value(serde_json::json!([
            {"st":{"c":"BTC-USD","d":false,"sz":1.0,"tr":90.0,"lim":null,"i":false}},
            {"l":{"c":"BTC-USD","b":true,"px":80.0,"sz":2.0,"tif":"GTC","r":false,"i":false}},
            {"rng":{"c":"BTC-USD","d":true,"sz":1.0,"pmin":70.0,"pmax":110.0,"lmin":null,"lmax":null,"i":false,"slSlippage":40.0,"tpSlippage":20.0}}
        ]))
        .unwrap();

        let mut expected = 12u32.to_le_bytes().to_vec();
        bincode::serialize_into(&mut expected, &actions).unwrap();
        expected.extend_from_slice(&42_u64.to_le_bytes());
        expected.extend_from_slice(account.as_ref());
        expected.push(SignatureDomain::Devnet as u8);

        assert_eq!(
            Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                account,
                42,
                &actions
            )
            .unwrap(),
            expected,
        );
        let bytes = bincode::serialize(&actions).unwrap();
        let decoded: Vec<Action> = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.len(), 3);
    }

    #[test]
    fn admin_actions_match_current_sdk_signing_vectors() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/sdk-admin-action-vectors.json")).unwrap();
        for vector in fixtures["vectors"].as_array().unwrap() {
            let actions: Vec<Action> = serde_json::from_value(vector["actions"].clone()).unwrap();
            let actual = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                Pubkey::default(),
                42,
                &actions,
            )
            .unwrap();
            let expected: Vec<u8> = vector["signable_hex"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(actual, expected, "{}", vector["actions"]);
            let encoded = bincode::serialize(&actions).unwrap();
            let decoded: Vec<Action> = bincode::deserialize(&encoded).unwrap();
            assert_eq!(
                serde_json::to_value(decoded).unwrap(),
                serde_json::to_value(actions).unwrap()
            );
        }
    }

    #[test]
    fn create_subaccounts_match_sdk_optional_margin_vectors() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/sdk-subaccount-vectors.json")).unwrap();
        for vector in fixtures["vectors"].as_array().unwrap() {
            let actions: Vec<Action> = serde_json::from_value(vector["actions"].clone()).unwrap();
            let signable = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                Pubkey::default(),
                42,
                &actions,
            )
            .unwrap();
            assert_eq!(
                bytes_hex(&signable),
                vector["signable_hex"].as_str().unwrap()
            );
            let encoded = bincode::serialize(&actions).unwrap();
            let restored: Vec<Action> = bincode::deserialize(&encoded).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), vector["actions"]);
        }
    }

    #[test]
    fn iso_transfers_match_current_sdk_vectors_at_every_depth() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/sdk-transfer-vectors.json")).unwrap();
        for vector in fixtures["vectors"].as_array().unwrap() {
            let actions: Vec<Action> = serde_json::from_value(vector["actions"].clone()).unwrap();
            let signable = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                Pubkey::default(),
                42,
                &actions,
            )
            .unwrap();
            assert!(signable.starts_with(&12u32.to_le_bytes()));
            assert_eq!(
                bytes_hex(&signable),
                vector["signable_hex"].as_str().unwrap()
            );
            let message = crate::transaction::ClearSignMessage::canonical_message_with_options(
                SignatureDomain::Devnet,
                Pubkey::default(),
                42,
                &actions,
                crate::transaction::ClearSignMessageOptions {
                    include_signable_schema: true,
                },
            )
            .unwrap();
            assert!(message.contains("transaction_version_le_u32"));
        }
    }

    #[test]
    fn raw_signable_matches_real_sdk_vectors() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/sdk-signing-vectors.json")).unwrap();
        for vector in fixtures["vectors"].as_array().unwrap() {
            let actual = Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
                SignatureDomain::Devnet,
                vector["account"].as_str().unwrap().parse().unwrap(),
                vector["nonce"].as_str().unwrap().parse().unwrap(),
                &serde_json::from_value::<Vec<Action>>(vector["actions"].clone()).unwrap(),
            )
            .unwrap();
            let expected: Vec<u8> = vector["signable_hex"]
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(actual, expected, "{}", vector["name"]);
            for (mut action, expected_id) in
                serde_json::from_value::<Vec<Action>>(vector["actions"].clone())
                    .unwrap()
                    .into_iter()
                    .zip(vector["default_meta_order_ids"].as_array().unwrap())
            {
                if let Some(expected_id) = expected_id.as_str() {
                    match &action {
                        Action::MarketOrder(order) => assert_eq!(
                            order.order_id(Pubkey::default(), 0, 0).to_string(),
                            expected_id
                        ),
                        Action::LimitOrder(order) => assert_eq!(
                            order.order_id(Pubkey::default(), 0, 0).to_string(),
                            expected_id
                        ),
                        _ => unreachable!(),
                    }
                    assert_eq!(action.hash().to_string(), expected_id);
                }
            }
        }
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
                    TransactionVersion::CURRENT, SignatureDomain::Testnet,
                    Pubkey::default(),
                    7,
                    &[action],
                )
                .expect("serialize trigger")
            ),
            "0c00000001000000000000000800000007000000000000004254432d5553440100a0724e1809000002000000000000000000000007000000000000004254432d555344014059730700000000000100000100000007000000000000004554482d55534400803424383a00000000c2eb0b00000000020000000100010000000000000000000000000000000000000000000000000000000000000000050700000000000000000000000000000000000000000000000000000000000000000000000000000002"
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
                    TransactionVersion::CURRENT, SignatureDomain::Testnet,
                    Pubkey::default(),
                    7,
                    &[action],
                )
                .expect("serialize on-fill")
            ),
            "0c00000001000000000000000a0000000100000007000000000000004554482d55534400803424383a00000000c2eb0b000000000200000001000002000000000000000000000007000000000000004254432d555344014059730700000000000100000000000007000000000000004254432d555344014059730700000000000101000000000000000000000000000000000000000000000000000000000000000005000700000000000000000000000000000000000000000000000000000000000000000000000000000002"
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
                builder_code: None,
                slippage: None,
                meta: ActionMeta::default(),
            })),
            actions: Vec::new(),
            meta: ActionMeta::default(),
        });

        assert!(
            Transaction::raw_signable_bytes(
                TransactionVersion::CURRENT,
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
            TransactionVersion::CURRENT,
            SignatureDomain::Devnet,
            Pubkey::default(),
            0,
            &[Action::UpdateLiquidatorConfig(LiqConfig::default())],
        )
        .expect("signable bytes");

        assert!(signable.starts_with(&12u32.to_le_bytes()));
        let ordinal_offset = 12u32.to_le_bytes().len() + 8;
        assert_eq!(
            u32::from_le_bytes(
                signable[ordinal_offset..ordinal_offset + 4]
                    .try_into()
                    .unwrap()
            ),
            43
        );
    }

    #[test]
    fn liquidator_updates_use_v12_at_any_depth_and_bind_apply_reserve() {
        let liquidator = Action::UpdateLiquidatorConfig(LiqConfig {
            instruments: vec![LiqConfigByInstrument {
                symbol: "BTC-USD".to_string(),
                apply_reserve: true,
                ..Default::default()
            }],
            ..Default::default()
        });
        let account = Pubkey::new_from_array([1; 32]);

        let direct = Transaction::raw_signable_bytes(
            TransactionVersion::CURRENT,
            SignatureDomain::Devnet,
            account,
            42,
            std::slice::from_ref(&liquidator),
        )
        .unwrap();
        assert!(direct.starts_with(&12u32.to_le_bytes()));

        let nested = Action::MultisigPropose(MultisigPropose {
            multisig: Pubkey::new_from_array([2; 32]),
            actions: vec![liquidator],
            proposal_lifetime_secs: None,
            meta: ActionMeta::default(),
        });
        let mut transaction = Transaction {
            version: crate::transaction::TransactionVersion::CURRENT,
            actions: vec![nested],
            nonce: 42,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };
        let signer = TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).unwrap();

        transaction.sign(&signer, SignatureDomain::Devnet).unwrap();
        assert!(transaction.verify(SignatureDomain::Devnet).unwrap());

        let Action::MultisigPropose(proposal) = &mut transaction.actions[0] else {
            unreachable!();
        };
        let Action::UpdateLiquidatorConfig(config) = &mut proposal.actions[0] else {
            unreachable!();
        };
        config.instruments[0].apply_reserve = false;

        assert!(!transaction.verify(SignatureDomain::Devnet).unwrap());
    }

    #[test]
    fn liquidator_signature_matches_current_sdk_vector() {
        let signer = TransactionSigner::from_private_key(TEST_PRIVATE_KEY1).unwrap();
        let account = signer.public_key();
        let instrument = |symbol: &str, max_exposure: f64| LiqConfigByInstrument {
            symbol: symbol.to_string(),
            execution_mode: Default::default(),
            dump_retry_secs: 60,
            max_exposure,
            reserve: 50.0,
            rfactor: 0.25,
            volume_percent: 5.0,
            volume_min: 1.0,
            volume_rampup: 0,
            max_sweep_bps: 100.0,
            max_adl_notional: 0.0,
            max_adl_percent: 0.0,
            apply_reserve: true,
        };
        let mut transaction = Transaction {
            version: crate::transaction::TransactionVersion::CURRENT,
            actions: vec![Action::UpdateLiquidatorConfig(LiqConfig {
                cross_exposure: 15e6,
                scoring_skew: 0.5,
                toxicity: 0.0,
                urgency_size_fraction: 0.25,
                sweep_sds: 2.0,
                price_to_sweep: true,
                instruments: vec![instrument("BTC-USD", 10e6), instrument("ETH-USD", 5e6)],
                meta: ActionMeta::default(),
            })],
            nonce: 42,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };

        transaction.sign(&signer, SignatureDomain::Devnet).unwrap();

        assert_eq!(
            transaction.signature.to_string(),
            "5J6EVwoRi6unyEZ3Zf686wKv4KtB1b3X2etGiPKMQyNmB1h96ds6MuGy4HRTsBSpoNjtPmg6iWM8wRAWEaofeMVy"
        );
    }

    #[test]
    fn market_admin_uses_sdk_ordinal_57() {
        let signable = Transaction::raw_signable_bytes(
            TransactionVersion::CURRENT,
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

        assert_eq!(u32::from_le_bytes(signable[12..16].try_into().unwrap()), 57);
    }

    #[test]
    fn update_account_policy_uses_sdk_ordinal_61() {
        let signable = Transaction::raw_signable_bytes(
            TransactionVersion::CURRENT,
            SignatureDomain::Devnet,
            Pubkey::default(),
            0,
            &[Action::UpdateAccountPolicy(
                crate::msgs::UpdateAccountPolicy {
                    withdraw_fee_usd: Some(2.0),
                    min_withdraw_usd: Some(7.0),
                    min_external_transfer_usd: Some(3.0),
                    meta: ActionMeta::default(),
                },
            )],
        )
        .expect("signable bytes");

        assert_eq!(u32::from_le_bytes(signable[12..16].try_into().unwrap()), 61);
    }

    #[test]
    fn account_policy_roundtrips_after_pre_deposit_ordinal() {
        let action = Action::UpdateAccountPolicy(crate::msgs::UpdateAccountPolicy {
            withdraw_fee_usd: Some(2.0),
            min_withdraw_usd: None,
            min_external_transfer_usd: Some(3.0),
            meta: ActionMeta::default(),
        });
        let encoded = bincode::serialize(&action).expect("serialize account policy");
        let decoded: Action = bincode::deserialize(&encoded).expect("deserialize account policy");
        assert!(matches!(decoded, Action::UpdateAccountPolicy(_)));
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
            TransactionVersion::CURRENT,
            SignatureDomain::Mainnet,
            tx.account,
            tx.nonce,
            &tx.actions,
        )
        .expect("mainnet bytes");
        let testnet = Transaction::raw_signable_bytes(
            TransactionVersion::CURRENT,
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

    // ───── LimitOrder ────────────────────────────────────────────────────────────────────────────

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
            version: crate::transaction::TransactionVersion::CURRENT,
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
            slippage: None,
            meta: ActionMeta {
                account,
                nonce: 43,
                seqno: 0,
                ..Default::default()
            },
        });

        let tx = Transaction {
            version: crate::transaction::TransactionVersion::CURRENT,
            actions: vec![action],
            nonce: 43,
            account,
            signer: Pubkey::default(),
            signature: Signature::default(),
        };

        (tx, signer)
    }

    #[test]
    fn market_order_without_slippage_uses_fixed_v12_fields() {
        let (tx, _) = make_market_order_tx();
        let current = Transaction::raw_signable_bytes(
            TransactionVersion::CURRENT,
            SignatureDomain::Devnet,
            tx.account,
            tx.nonce,
            &tx.actions,
        )
        .expect("serialize current market signing payload");

        let mut legacy = 12u32.to_le_bytes().to_vec();
        legacy.extend_from_slice(&1u64.to_le_bytes());
        legacy.extend_from_slice(&0u32.to_le_bytes());
        legacy.extend_from_slice(&7u64.to_le_bytes());
        legacy.extend_from_slice(b"BTC-USD");
        legacy.push(0);
        legacy.extend_from_slice(&25_000_000u64.to_le_bytes());
        legacy.push(0);
        legacy.push(0);
        legacy.push(0); // absent builder code
        legacy.push(0); // absent slippage
        legacy.extend_from_slice(&43u64.to_le_bytes());
        legacy.extend_from_slice(tx.account.as_ref());
        legacy.push(SignatureDomain::Devnet as u8);

        assert_eq!(current, legacy);
    }

    #[test]
    fn market_order_slippage_is_signed_and_tamper_evident() {
        let (mut tx, signer) = make_market_order_tx();
        let Action::MarketOrder(order) = &mut tx.actions[0] else {
            unreachable!("expected market order");
        };
        order.slippage = Some(25.0);

        tx.sign(&signer, SignatureDomain::Devnet)
            .expect("sign slippage market order");
        assert!(tx
            .verify(SignatureDomain::Devnet)
            .expect("verify slippage market order"));

        let Action::MarketOrder(order) = &mut tx.actions[0] else {
            unreachable!("expected market order");
        };
        order.slippage = Some(26.0);
        assert!(!tx
            .verify(SignatureDomain::Devnet)
            .expect("verify tampered slippage market order"));
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
            version: crate::transaction::TransactionVersion::CURRENT,
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

    // ───── CancelAll ─────────────────────────────────────────────────────────────────────────────

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
            version: crate::transaction::TransactionVersion::CURRENT,
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

    // ───── Faucet (no amount) ────────────────────────────────────────────────────────────────────

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
            version: crate::transaction::TransactionVersion::CURRENT,
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

    // ───── TakeProfit (market trigger, no limit) ─────────────────────────────────────────────────

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
            builder_code: None,
            slippage: None,
            meta: Default::default(),
        });

        let tx = Transaction {
            version: crate::transaction::TransactionVersion::CURRENT,
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
            builder_code: None,
            slippage: None,
            meta: Default::default(),
        });

        let tx = Transaction {
            version: crate::transaction::TransactionVersion::CURRENT,
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
