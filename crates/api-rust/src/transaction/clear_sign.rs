use crate::msgs::conditional::{OnFill, Range, StopOrTP, Trailing, Trigger};
use crate::msgs::multisig::{CreateMultisig, UpdateMultisigPolicy};
use crate::msgs::UpdateUserSettings;
use crate::transaction::{Action, SignatureDomain, Transaction};
use solana_pubkey::Pubkey;
use std::fmt::Write as _;

/// Controls optional fields rendered in a clear-sign message.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClearSignMessageOptions {
    /// Includes the canonical signing schema in the rendered message.
    pub include_signable_schema: bool,
}

/// Builds canonical human-readable messages for Bulk transaction signing.
pub struct ClearSignMessage;

impl ClearSignMessage {
    // ───── Public API Contract ─────────────────────────────────────────────────────────────────

    /// Builds the canonical clear-sign message for a transaction.
    ///
    /// # Arguments
    /// * `signature_domain` - Network domain bound to the transaction signature.
    /// * `account` - Account whose transaction actions will be executed.
    /// * `nonce` - Transaction nonce included in the canonical signing bytes.
    /// * `actions` - Ordered actions included in the transaction.
    ///
    /// # Returns
    /// The canonical clear-sign message expected by the Bulk verifier.
    pub fn canonical_message(
        signature_domain: SignatureDomain,
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
    ) -> eyre::Result<String> {
        Self::canonical_message_with_options(
            signature_domain,
            account,
            nonce,
            actions,
            ClearSignMessageOptions::default(),
        )
    }

    /// Builds the canonical clear-sign message with explicit rendering options.
    ///
    /// # Arguments
    /// * `signature_domain` - Network domain bound to the transaction signature.
    /// * `account` - Account whose transaction actions will be executed.
    /// * `nonce` - Transaction nonce included in the canonical signing bytes.
    /// * `actions` - Ordered actions included in the transaction.
    /// * `options` - Optional canonical-message rendering controls.
    ///
    /// # Returns
    /// The canonical clear-sign message expected by the Bulk verifier.
    pub fn canonical_message_with_options(
        signature_domain: SignatureDomain,
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
        options: ClearSignMessageOptions,
    ) -> eyre::Result<String> {
        let signable = Transaction::raw_signable_bytes(signature_domain, account, nonce, actions)?;
        Ok(Self::render_message(
            signature_domain,
            account,
            nonce,
            actions,
            signable.as_slice(),
            options,
        ))
    }

    fn render_message(
        signature_domain: SignatureDomain,
        account: Pubkey,
        nonce: u64,
        actions: &[Action],
        signable: &[u8],
        options: ClearSignMessageOptions,
    ) -> String {
        let mut message = String::with_capacity(256 + actions.len().saturating_mul(96));
        let _ = writeln!(message, "Bulk Exchange Transaction");
        let _ = writeln!(message, "Network: {signature_domain}");
        let _ = writeln!(message, "Account: {account}");
        let _ = writeln!(message, "Nonce: {nonce}");
        let _ = writeln!(message, "Actions: {}", actions.len());
        let _ = writeln!(message, "Signable-Hash: {}", Self::sha256_hex(signable));
        if options.include_signable_schema {
            let _ = writeln!(
            message,
            "Signable-Schema: bincode(commission_signable_actions)||nonce_le_u64||account_bytes||signature_domain_u8"
        );
        }
        for (index, action) in actions.iter().enumerate() {
            Self::render_action(&mut message, action, index.to_string());
        }
        message
    }

    fn render_action(message: &mut String, action: &Action, path: String) {
        let _ = writeln!(message, "[{path}] {}", Self::action_line(action));
        if let Action::MultisigPropose(proposal) = action {
            for (index, nested_action) in proposal.actions.iter().enumerate() {
                Self::render_action(message, nested_action, format!("{path}.{index}"));
            }
        }
    }

    fn sha256_hex(payload: &[u8]) -> String {
        use sha2::Digest as _;
        let digest = sha2::Sha256::digest(payload);
        let mut hex = String::with_capacity(digest.len().saturating_mul(2));
        for byte in digest.as_slice() {
            let _ = write!(hex, "{:02x}", byte);
        }
        hex
    }

    fn fmt_opt(value: Option<f64>) -> String {
        value
            .map(|number| format!("{number:.8}"))
            .unwrap_or_else(|| "-".to_string())
    }

    fn action_line(action: &Action) -> String {
        match action {
        Action::MarketOrder(order) => format!(
            "Market {} {} sz={:.8} ro={} iso={}",
            order.symbol,
            if order.is_buy { "Buy" } else { "Sell" },
            order.size,
            order.reduce_only,
            order.iso,
        ),
        Action::LimitOrder(order) => format!(
            "Limit {} {} px={:.8} sz={:.8} tif={:?} ro={} iso={}",
            order.symbol,
            if order.is_buy { "Buy" } else { "Sell" },
            order.price,
            order.size,
            order.tif,
            order.reduce_only,
            order.iso,
        ),
        Action::ModifyOrder(order) => {
            format!(
                "Modify {} oid={} sz={:.8}",
                order.symbol, order.order_id, order.amount
            )
        }
        Action::Cancel(order) => format!("Cancel {} oid={}", order.symbol, order.oid),
        Action::CancelAll(order) => {
            if order.symbols.is_empty() {
                "CancelAll *".to_string()
            } else {
                format!("CancelAll {}", order.symbols.join(","))
            }
        }
        Action::Stop(order) => Self::stop_tp("Stop", order),
        Action::TakeProfit(order) => Self::stop_tp("TakeProfit", order),
        Action::Range(order) => Self::range(order),
        Action::Trigger(order) => Self::trigger(order),
        Action::Trailing(order) => Self::trailing(order),
        Action::OnFill(order) => Self::on_fill(order),
        Action::Faucet(action) => format!(
            "Faucet user={} amount={}",
            action.user,
            action
                .amount
                .map(|amount| format!("{amount:.8}"))
                .unwrap_or_else(|| "-".to_string())
        ),
        Action::AgentWalletCreation(action) => {
            format!(
                "AgentWallet agent={} delete={}",
                action.agent, action.delete
            )
        }
        Action::UpdateUserSettings(action) => Self::user_settings(action),
        Action::CreateSubAccount(action) => format!(
            "CreateSubAccount name={} amt={}",
            action.name,
            action
                .margin_amount
                .map(|value| format!("{value:.8}"))
                .unwrap_or_else(|| "-".to_string())
        ),
        Action::RemoveSubAccount(action) => format!("RemoveSubAccount {}", action.to_remove),
        Action::RenameSubAccount(action) => {
            format!(
                "RenameSubAccount account={} name={}",
                action.account, action.name
            )
        }
        Action::Transfer(action) => format!(
            "Transfer {:?} from={} to={} amt={:.8}",
            action.kind, action.from, action.to, action.margin_amount,
        ),
        Action::CreateMultisig(action) => Self::create_multisig(action),
        Action::MultisigPropose(action) => format!(
            "MultisigPropose {} nested={} life={:?}",
            action.multisig,
            action.actions.len(),
            action.proposal_lifetime_secs
        ),
        Action::MultisigApprove(action) => {
            format!(
                "MultisigApprove {} prop={}",
                action.multisig, action.proposal_id
            )
        }
        Action::MultisigReject(action) => {
            format!(
                "MultisigReject {} prop={}",
                action.multisig, action.proposal_id
            )
        }
        Action::MultisigCancel(action) => {
            format!(
                "MultisigCancel {} prop={}",
                action.multisig, action.proposal_id
            )
        }
        Action::MultisigExecute(action) => {
            format!(
                "MultisigExecute {} prop={}",
                action.multisig, action.proposal_id
            )
        }
        Action::UpdateMultisigPolicy(action) => Self::update_multisig(action),
        Action::WhitelistFaucet(action) => {
            format!(
                "WhitelistFaucet target={} whitelist={}",
                action.target, action.whitelist
            )
        }
        Action::AddMarket(action) => format!("AddMarket {}", action.symbol),
        Action::MarketAdmin(action) => {
            format!(
                "MarketAdmin {} {:?} price={}",
                action.symbol,
                action.action,
                Self::fmt_opt(action.price)
            )
        }
        Action::PricingAdmin(action) => {
            format!("PricingAdmin {} {:?}", action.instrument, action.source)
        }
        Action::ConfigFairPrice(action) => format!(
            "ConfigFairPrice payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigVolatility(action) => format!(
            "ConfigVolatility payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigSecurity(action) => format!(
            "ConfigSecurity payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigRegime(action) => format!(
            "ConfigRegime payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigRisk(action) => format!(
            "ConfigRiskMatrix payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigFeePolicy(action) => format!(
            "ConfigFeePolicy payload={}",
            bs58::encode(action.payload.as_slice()).into_string()
        ),
        Action::ConfigFunding(action) => format!(
            "ConfigFunding symbol={} rate={} deviationCap={} fundingCap={} premiumHorizon={} notional={} samplePeriod={} meanWindow={}",
            action.symbol,
            action.rate,
            action.deviation_cap,
            action.funding_cap,
            action.premium_horizon,
            action.notional,
            action.sample_period,
            action.mean_window,
        ),
        Action::Price(action) => format!(
            "Price asset={} px={:.8} ts={}",
            action.asset, action.price, action.timestamp
        ),
        Action::PythOracle(action) => format!("PythOracle count={}", action.oracles.len()),
        Action::Corrs(action) => format!(
            "Corrs index={} rows={}",
            action.index.join(","),
            action.matrix.len()
        ),
        Action::Beacon(action) => format!(
            "Beacon epoch={} wall_clock_ns={} since_commit_us={}",
            action.epoch, action.wall_clock_ns, action.since_commit_us
        ),
        Action::Join(action) => format!("Join committed_round={}", action.committed_round),
        Action::UpdateValidatorSet(action) => format!(
            "UpdateValidatorSet version={} add={} remove={} admin_sigs={}",
            action.version,
            action.added.len(),
            action.removed.len(),
            action.admin_sigs.len()
        ),
        Action::UpdateRiskConfig(action) => format!(
            "UpdateRiskConfig settle_ccy={} max_loss={:.8} eloss_floor={:.8} max_pliq={:.8} margin_buffer={:.8} corr_discount={:.8} cascade_factor={:.8}",
            action.settle_ccy,
            action.max_loss,
            action.eloss_floor,
            action.max_pliq,
            action.margin_buffer,
            action.corr_discount,
            action.cascade_factor
        ),
        Action::UpdateAccountPolicy(action) => format!(
            "UpdateAccountPolicy withdraw_fee_usd={:?} min_withdraw_usd={:?} min_external_transfer_usd={:?}",
            action.withdraw_fee_usd,
            action.min_withdraw_usd,
            action.min_external_transfer_usd
        ),
        Action::UserAdmin(action) => format!(
            "UserAdmin pubkey={} maxorders={:?} globalMaxorders={:?}",
            action.pubkey, action.maxorders, action.global_maxorders
        ),
        Action::ApproveCommissionFee(action) => {
            format!(
                "ApproveCommissionFee to={} max_fee={}",
                action.to, action.max_fee
            )
        }
        Action::RevokeCommissionFee(action) => {
            format!("RevokeCommissionFee to={}", action.to)
        }
        Action::RewardSettlement(action) => {
            let total_ppm = action
                .weights
                .iter()
                .fold(0u32, |total, (_, weight)| total.saturating_add(*weight));
            format!(
                "RewardSettlement epoch={} weights={} total_ppm={} recipients={}",
                action.epoch,
                action.weights.len(),
                total_ppm,
                action
                    .weights
                    .iter()
                    .map(|(recipient, weight)| format!("{recipient}:{weight}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Action::UpdateLiquidatorConfig(action) => format!(
            "UpdateLiquidatorConfig cross_exposure={:.8} scoring_skew={:.8} toxicity={:.8} urgency_size_fraction={:.8} sweep_sds={:.8} price_to_sweep={} instruments={}",
            action.cross_exposure,
            action.scoring_skew,
            action.toxicity,
            action.urgency_size_fraction,
            action.sweep_sds,
            action.price_to_sweep,
            action.instruments.len()
        ),
        Action::Deposit(action) => format!(
            "Deposit user={} vault={} amount={} solana_sig={} ix={}",
            action.user,
            action.vault,
            action.amount,
            action.solana_signature,
            action.instruction_index
        ),
        Action::Withdraw(action) => format!(
            "Withdraw user={} vault={} rta={} amount={}",
            action.user, action.vault, action.recipient_token_account, action.amount
        ),
        Action::WithdrawConfirmation(action) => format!(
            "WithdrawConfirmation user_token_account={} hash={} signature={} ix={}",
            action.user_token_account, action.hash, action.signature, action.instruction_index
        ),
        Action::NonceCommitment(action) => format!(
            "NonceCommitment node={} hiding={:?} binding={:?} session_id={}",
            action.signer, action.hiding, action.binding, action.session_id
        ),
        Action::PartialSignature(action) => format!(
            "PartialSignature node={} session_id={} share={:?}",
            action.signer, action.session_id, action.share
        ),
        Action::DkgRound1(action) => format!("DkgRound1 signer={}", action.signer,),
        Action::InitializeVault(action) => format!(
            "InitializeVault vault={} mint={} token_account={} solana_sig={} ix={}",
            action.vault,
            action.mint,
            action.token_account,
            action.solana_signature,
            action.instruction_index
        ),
        Action::UpdateFrostGroup(action) => format!(
            "UpdateFrostGroup state={} frost_group_key={} solana_sig={} ix={}",
            action.state,
            action.frost_group_key,
            action.solana_signature,
            action.instruction_index
        ),
        Action::WithdrawSubmitted(action) => format!(
            "WithdrawSubmitted hash={} signature={} recipient={} vault={} amount={} blockhash={}",
            action.hash,
            action.signature,
            action.recipient_token_account,
            action.vault,
            action.amount,
            action.blockhash
        ),
        Action::WithdrawFailed(action) => {
            format!("WithdrawFailed hash={} reason={}", action.hash, action.reason)
        }
        Action::FrostWithdrawStart(action) => {
            format!(
                "FrostWithdrawStart hash={} recipient={} vault={} amount={}",
                action.hash, action.recipient_token_account, action.vault, action.amount
            )
        }
        Action::ActivateProtocolVersion(action) => {
            format!("ActivateProtocolVersion version={}", action.version)
        }
        Action::RevokePendingActivation(action) => {
            format!("RevokePendingActivation version={}", action.version)
        }
        Action::DkgFinished(action) => {
            format!("DkgFinished signer={} epoch={}", action.signer, action.epoch)
        }
        Action::SolanaBlockAnchor(action) => {
            format!(
                "SolanaBlockAnchor slot={} blockhash={}",
                action.slot, action.blockhash
            )
        }
        Action::PreDepositCredit(action) => format!(
            "PreDepositCredit user={} vault={} amount={} slot={} pda={} entry={}",
            action.user,
            action.vault,
            action.amount,
            action.migration_slot,
            action.pre_deposit_pda,
            action.entry_index
        ),
        Action::ConfigMakerRebateTier(action) => format!(
            "ConfigMakerRebateTier instrument={} maker={} minimum_tier={:?} expires_slot={:?}",
            action.instrument, action.maker, action.minimum_tier, action.expires_slot
        ),
    }
    }

    fn stop_tp(kind: &str, action: &StopOrTP) -> String {
        format!(
            "{} {} {} thresh={:.8} sz={:.8} limit={}",
            kind,
            action.symbol,
            if action.is_above { "Above" } else { "Below" },
            action.threshold,
            action.size,
            Self::fmt_opt(action.limit),
        )
    }

    fn range(action: &Range) -> String {
        format!(
            "Range {} {} min={:.8} max={:.8} sz={:.8} lmin={} lmax={}",
            action.symbol,
            if action.is_buy { "Buy" } else { "Sell" },
            action.collar_min,
            action.collar_max,
            action.size,
            Self::fmt_opt(action.limit_min),
            Self::fmt_opt(action.limit_max),
        )
    }

    fn trigger(action: &Trigger) -> String {
        format!(
            "Trigger {} {} thresh={:.8} nested={}",
            action.symbol,
            if action.is_above { "Above" } else { "Below" },
            action.threshold,
            action.actions.len(),
        )
    }

    fn trailing(action: &Trailing) -> String {
        format!(
            "Trailing {} {} sz={:.8} trail={}bps step={}bps limit={}",
            action.symbol,
            if action.is_buy { "Buy" } else { "Sell" },
            action.size,
            action.trail_bps,
            action.step_bps,
            Self::fmt_opt(action.limit),
        )
    }

    fn on_fill(action: &OnFill) -> String {
        format!(
            "OnFill parent={:?} nested={}",
            action.trigger,
            action.actions.len()
        )
    }

    fn user_settings(action: &UpdateUserSettings) -> String {
        let mut pairs: Vec<_> = action.max_leverage.iter().collect();
        pairs.sort_by(|left, right| left.0.cmp(right.0));
        let body = pairs
            .iter()
            .map(|(symbol, leverage)| format!("{}:{leverage:.8}", symbol))
            .collect::<Vec<_>>()
            .join(",");
        format!("UpdateLeverage {body}")
    }

    fn create_multisig(action: &CreateMultisig) -> String {
        format!(
            "CreateMultisig thresh={} lock={} life={} signers={}",
            action.threshold,
            action.time_lock_secs,
            action.proposal_lifetime_secs,
            action
                .signers
                .iter()
                .map(|pubkey| pubkey.to_string())
                .collect::<Vec<_>>()
                .join(","),
        )
    }

    fn update_multisig(action: &UpdateMultisigPolicy) -> String {
        format!(
            "UpdateMultisig {} thresh={} lock={} life={} signers={}",
            action.multisig,
            action
                .threshold
                .map_or_else(|| "unchanged".to_string(), |value| value.to_string()),
            action
                .time_lock_secs
                .map_or_else(|| "unchanged".to_string(), |value| value.to_string()),
            action
                .proposal_lifetime_secs
                .map_or_else(|| "unchanged".to_string(), |value| value.to_string()),
            action
                .signers
                .as_ref()
                .map(|signers| signers
                    .iter()
                    .map(|pubkey| pubkey.to_string())
                    .collect::<Vec<_>>()
                    .join(","))
                .unwrap_or_else(|| "unchanged".to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::ClearSignMessage;
    use crate::common::tif::TimeInForce;
    use crate::msgs::{BuilderCode, Faucet, LimitOrder, OpaqueAction, UpdateMultisigPolicy};
    use crate::transaction::{Action, ActionMeta, SignatureDomain};
    use solana_pubkey::Pubkey;
    use std::sync::Arc;

    struct ClearSignTest;

    impl ClearSignTest {
        fn signable_hash_line(message: &str) -> &str {
            message
                .lines()
                .find(|line| line.starts_with("Signable-Hash: "))
                .expect("missing signable hash line")
        }
    }

    #[test]
    fn partial_multisig_update_renders_unchanged_fields() {
        let multisig = Pubkey::new_unique();
        let message = ClearSignMessage::update_multisig(&UpdateMultisigPolicy {
            multisig,
            signers: None,
            threshold: Some(3),
            time_lock_secs: None,
            proposal_lifetime_secs: None,
            meta: ActionMeta::default(),
        });

        assert_eq!(
            message,
            format!(
                "UpdateMultisig {multisig} thresh=3 lock=unchanged life=unchanged signers=unchanged"
            )
        );
    }

    #[test]
    fn message_is_deterministic() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100_000.0,
            size: 0.1,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta::default(),
        })];
        let first = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions.as_slice(),
        )
        .expect("build message");
        let second = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions.as_slice(),
        )
        .expect("build message");
        assert_eq!(first, second);
    }

    #[test]
    fn message_displays_and_binds_signature_domain() {
        let account = Pubkey::new_unique();
        let actions = [Action::Faucet(Faucet {
            user: account,
            amount: None,
            meta: ActionMeta::default(),
        })];
        let mainnet =
            ClearSignMessage::canonical_message(SignatureDomain::Mainnet, account, 42, &actions)
                .expect("mainnet message");
        let testnet =
            ClearSignMessage::canonical_message(SignatureDomain::Testnet, account, 42, &actions)
                .expect("testnet message");

        assert!(mainnet.contains("Network: mainnet"));
        assert!(testnet.contains("Network: testnet"));
        assert_ne!(
            ClearSignTest::signable_hash_line(&mainnet),
            ClearSignTest::signable_hash_line(&testnet)
        );
    }

    #[test]
    fn message_contains_expected_fields() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::Faucet(Faucet {
            user: account,
            amount: None,
            meta: ActionMeta::default(),
        })];
        let message = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions.as_slice(),
        )
        .expect("build message");
        assert!(message.contains("Bulk Exchange Transaction"));
        assert!(message.contains(&format!("Account: {account}")));
        assert!(message.contains("Nonce: 42"));
        assert!(message.contains("Faucet"));
        assert!(message.contains("Signable-Hash: "));
        assert!(!message.contains("Signable-Schema:"));
    }

    #[test]
    fn message_shows_limit_order_fields() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::LimitOrder(LimitOrder {
            symbol: Arc::from("ETH-USD"),
            is_buy: false,
            price: 3500.0,
            size: 1.5,
            tif: TimeInForce::GTC,
            reduce_only: true,
            iso: false,
            builder_code: None,
            meta: ActionMeta::default(),
        })];
        let message = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            99,
            actions.as_slice(),
        )
        .expect("build message");
        assert!(message.contains("ETH-USD"));
        assert!(message.contains("Sell"));
        assert!(message.contains("3500.00000000"));
        assert!(message.contains("1.50000000"));
    }

    #[test]
    fn message_binds_builder_code_through_signable_hash() {
        let account = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let order = LimitOrder {
            symbol: Arc::from("ETH-USD"),
            is_buy: false,
            price: 3500.0,
            size: 1.5,
            tif: TimeInForce::GTC,
            reduce_only: true,
            iso: false,
            builder_code: Some(BuilderCode {
                to: recipient,
                fee: 5,
            }),
            meta: ActionMeta::default(),
        };
        let message = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            99,
            &[Action::LimitOrder(order.clone())],
        )
        .expect("build message");
        let without_builder = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            99,
            &[Action::LimitOrder(LimitOrder {
                builder_code: None,
                ..order
            })],
        )
        .expect("build message without builder code");

        assert!(!message.contains(&recipient.to_string()));
        assert_ne!(
            ClearSignTest::signable_hash_line(&message),
            ClearSignTest::signable_hash_line(&without_builder)
        );
    }

    #[test]
    fn message_displays_multisig_proposal_actions_recursively() {
        use crate::msgs::MultisigPropose;

        let account = Pubkey::new_unique();
        let multisig = Pubkey::new_unique();
        let nested_multisig = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let actions = [Action::MultisigPropose(MultisigPropose {
            multisig,
            actions: vec![
                Action::Faucet(Faucet {
                    user: recipient,
                    amount: Some(2.0),
                    meta: ActionMeta::default(),
                }),
                Action::MultisigPropose(MultisigPropose {
                    multisig: nested_multisig,
                    actions: vec![Action::Faucet(Faucet {
                        user: recipient,
                        amount: Some(3.0),
                        meta: ActionMeta::default(),
                    })],
                    proposal_lifetime_secs: Some(60),
                    meta: ActionMeta::default(),
                }),
            ],
            proposal_lifetime_secs: None,
            meta: ActionMeta::default(),
        })];

        let message =
            ClearSignMessage::canonical_message(SignatureDomain::Devnet, account, 42, &actions)
                .expect("build message");

        assert!(message.contains(&format!(
            "[0] MultisigPropose {multisig} nested=2 life=None"
        )));
        assert!(message.contains(&format!("[0.0] Faucet user={recipient} amount=2.00000000")));
        assert!(message.contains(&format!(
            "[0.1] MultisigPropose {nested_multisig} nested=1 life=Some(60)"
        )));
        assert!(message.contains(&format!(
            "[0.1.0] Faucet user={recipient} amount=3.00000000"
        )));
    }

    #[test]
    fn message_binds_full_precision_values_beyond_display_rounding() {
        let account = Pubkey::new_unique();
        let actions_one = vec![Action::Faucet(Faucet {
            user: account,
            amount: Some(1.0000000001),
            meta: ActionMeta::default(),
        })];
        let actions_two = vec![Action::Faucet(Faucet {
            user: account,
            amount: Some(1.0000000002),
            meta: ActionMeta::default(),
        })];
        let msg_one = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions_one.as_slice(),
        )
        .expect("one");
        let msg_two = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions_two.as_slice(),
        )
        .expect("two");
        assert_ne!(msg_one, msg_two);
        assert!(msg_one.contains("amount=1.00000000"));
        assert!(msg_two.contains("amount=1.00000000"));
        assert_ne!(
            ClearSignTest::signable_hash_line(msg_one.as_str()),
            ClearSignTest::signable_hash_line(msg_two.as_str())
        );
    }

    #[test]
    fn message_base58_encodes_opaque_payload_preview() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::ConfigRisk(OpaqueAction {
            payload: vec![7; 128],
            meta: ActionMeta::default(),
        })];
        let message = ClearSignMessage::canonical_message(
            SignatureDomain::Devnet,
            account,
            42,
            actions.as_slice(),
        )
        .expect("build message");

        assert!(message.contains(&format!(
            "ConfigRiskMatrix payload={}",
            bs58::encode([7; 128]).into_string()
        )));
    }
}
