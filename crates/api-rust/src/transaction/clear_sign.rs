use crate::msgs::conditional::{OnFill, Range, StopOrTP, Trailing, Trigger};
use crate::msgs::multisig::{CreateMultisig, UpdateMultisigPolicy};
use crate::msgs::UpdateUserSettings;
use crate::transaction::{Action, SignatureDomain, Transaction};
use solana_pubkey::Pubkey;
use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, Default)]
pub struct ClearSignMessageOptions {
    pub include_signable_schema: bool,
}

pub fn canonical_message(
    signature_domain: SignatureDomain,
    account: Pubkey,
    nonce: u64,
    actions: &[Action],
) -> eyre::Result<String> {
    canonical_message_with_options(
        signature_domain,
        account,
        nonce,
        actions,
        ClearSignMessageOptions::default(),
    )
}

pub fn canonical_message_with_options(
    signature_domain: SignatureDomain,
    account: Pubkey,
    nonce: u64,
    actions: &[Action],
    options: ClearSignMessageOptions,
) -> eyre::Result<String> {
    let signable = Transaction::raw_signable_bytes(signature_domain, account, nonce, actions)?;
    let mut message = String::with_capacity(256 + actions.len().saturating_mul(96));
    let _ = writeln!(message, "Bulk Exchange Transaction");
    let _ = writeln!(message, "Network: {signature_domain}");
    let _ = writeln!(message, "Account: {account}");
    let _ = writeln!(message, "Nonce: {nonce}");
    let _ = writeln!(message, "Actions: {}", actions.len());
    let _ = writeln!(
        message,
        "Signable-Hash: {}",
        sha256_hex(signable.as_slice())
    );
    if options.include_signable_schema {
        let _ = writeln!(
            message,
            "Signable-Schema: bincode(commission_signable_actions)||nonce_le_u64||account_bytes||signature_domain_u8"
        );
    }
    for (index, action) in actions.iter().enumerate() {
        let _ = writeln!(message, "[{}] {}", index, action_line(action));
    }
    Ok(message)
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

fn builder_code(builder_code: Option<crate::msgs::order::BuilderCode>) -> String {
    builder_code
        .map(|builder_code| {
            format!(
                " builder_code_to={} builder_code_fee={}bps",
                builder_code.to, builder_code.fee
            )
        })
        .unwrap_or_default()
}

fn opaque_payload(kind: &str, payload: &[u8]) -> String {
    format!(
        "{kind} payload_len={} payload_sha256={}",
        payload.len(),
        sha256_hex(payload)
    )
}

fn action_line(action: &Action) -> String {
    match action {
        Action::MarketOrder(order) => format!(
            "Market {} {} sz={:.8} ro={} iso={}{}",
            order.symbol,
            if order.is_buy { "Buy" } else { "Sell" },
            order.size,
            order.reduce_only,
            order.iso,
            builder_code(order.builder_code),
        ),
        Action::LimitOrder(order) => format!(
            "Limit {} {} px={:.8} sz={:.8} tif={:?} ro={} iso={}{}",
            order.symbol,
            if order.is_buy { "Buy" } else { "Sell" },
            order.price,
            order.size,
            order.tif,
            order.reduce_only,
            order.iso,
            builder_code(order.builder_code),
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
        Action::Stop(order) => stop_tp("Stop", order),
        Action::TakeProfit(order) => stop_tp("TakeProfit", order),
        Action::Range(order) => range(order),
        Action::Trigger(order) => trigger(order),
        Action::Trailing(order) => trailing(order),
        Action::OnFill(order) => on_fill(order),
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
        Action::UpdateUserSettings(action) => user_settings(action),
        Action::CreateSubAccount(action) => format!(
            "CreateSubAccount name={} amt={}",
            action.name,
            action
                .margin_amount
                .map(|value| format!("{value:.8}"))
                .unwrap_or_else(|| "-".to_string())
        ),
        Action::RemoveSubAccount(action) => format!("RemoveSubAccount {}", action.to_remove),
        Action::Transfer(action) => format!(
            "Transfer {:?} from={} to={} amt={:.8}",
            action.kind, action.from, action.to, action.margin_amount,
        ),
        Action::CreateMultisig(action) => create_multisig(action),
        Action::MultisigPropose(action) => format!(
            "MultisigPropose {} nested={}",
            action.multisig,
            action.actions.len()
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
        Action::UpdateMultisigPolicy(action) => update_multisig(action),
        Action::AdminOp(action) => format!(
            "AdminOp actions={} [{}]",
            action.actions.len(),
            action
                .actions
                .iter()
                .map(action_line)
                .collect::<Vec<_>>()
                .join(" | ")
        ),
        Action::WhitelistFaucet(action) => {
            format!(
                "WhitelistFaucet target={} whitelist={}",
                action.target, action.whitelist
            )
        }
        Action::AddMarket(action) => format!("AddMarket {}", action.symbol),
        Action::MarketAdmin(action) => format!(
            "MarketAdmin {} {:?} price={}",
            action.symbol,
            action.action,
            fmt_opt(action.price)
        ),
        Action::PricingAdmin(action) => {
            format!("PricingAdmin {} {:?}", action.instrument, action.source)
        }
        Action::FrostWithdrawStart(action) => format!(
            "FrostWithdrawStart hash={} recipient={} vault={} amount={}",
            action.hash, action.recipient_token_account, action.vault, action.amount
        ),
        Action::RewardSettlement(action) => format!(
            "RewardSettlement epoch={} validators={}",
            action.epoch,
            action.weights.len()
        ),
        Action::Deposit(action) => format!("Deposit user={} amount={}", action.user, action.amount),
        Action::Withdraw(action) => {
            format!("Withdraw user={} amount={}", action.user, action.amount)
        }
        Action::WithdrawConfirmation(action) => {
            format!("WithdrawConfirmation hash={}", action.hash)
        }
        Action::NonceCommitment(action) => {
            format!(
                "NonceCommitment signer={} session={}",
                action.signer, action.session_id
            )
        }
        Action::PartialSignature(action) => {
            format!(
                "PartialSignature signer={} session={}",
                action.signer, action.session_id
            )
        }
        Action::WithdrawSubmitted(action) => format!("WithdrawSubmitted hash={}", action.hash),
        Action::WithdrawFailed(action) => {
            format!(
                "WithdrawFailed hash={} reason={}",
                action.hash, action.reason
            )
        }
        Action::DkgRound1(action) => {
            format!("DkgRound1 signer={} epoch={}", action.signer, action.epoch)
        }
        Action::InitializeVault(action) => format!("InitializeVault vault={}", action.vault),
        Action::UpdateFrostGroup(action) => {
            format!("UpdateFrostGroup state={}", action.state)
        }
        Action::DkgFinished(action) => {
            format!(
                "DkgFinished signer={} epoch={}",
                action.signer, action.epoch
            )
        }
        Action::SolanaBlockAnchor(action) => {
            format!(
                "SolanaBlockAnchor slot={} blockhash={}",
                action.slot, action.blockhash
            )
        }
        Action::ConfigMakerRebateTier(action) => format!(
            "ConfigMakerRebateTier {} maker={} tier={:?} expires={:?}",
            action.instrument, action.maker, action.minimum_tier, action.expires_slot
        ),
        Action::ConfigFairPrice(action) => opaque_payload("ConfigFairPrice", &action.payload),
        Action::ConfigVolatility(action) => opaque_payload("ConfigVolatility", &action.payload),
        Action::ConfigSecurity(action) => opaque_payload("ConfigSecurity", &action.payload),
        Action::ConfigRegime(action) => opaque_payload("ConfigRegime", &action.payload),
        Action::ConfigRisk(action) => opaque_payload("ConfigRiskMatrix", &action.payload),
        Action::ConfigFeePolicy(action) => opaque_payload("ConfigFeePolicy", &action.payload),
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
        Action::RenameSubAccount(action) => {
            format!(
                "RenameSubAccount account={} name={}",
                action.account, action.name
            )
        }
        Action::UpdateValidatorSet(action) => format!(
            "UpdateValidatorSet version={} add={} remove={} admin_sigs={}",
            action.version,
            action.added.len(),
            action.removed.len(),
            action.admin_sigs.len()
        ),
        Action::UpdateRiskConfig(action) => format!("UpdateRiskConfig {:?}", action),
        Action::ApproveCommissionFee(action) => {
            format!(
                "ApproveBuilderCode to={} fee={}bps",
                action.to, action.max_fee
            )
        }
        Action::RevokeCommissionFee(action) => {
            format!("RevokeBuilderCode to={}", action.to)
        }
        Action::UpdateLiquidatorConfig(action) => format!("UpdateLiquidatorConfig({:?}", action),
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
        fmt_opt(action.limit),
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
        fmt_opt(action.limit_min),
        fmt_opt(action.limit_max),
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
        fmt_opt(action.limit),
    )
}

fn on_fill(action: &OnFill) -> String {
    format!(
        "OnFill trigger=({}) nested={}",
        action_line(&action.trigger),
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

#[cfg(test)]
mod tests {
    use super::canonical_message;
    use crate::common::tif::TimeInForce;
    use crate::msgs::{BuilderCode, Faucet, LimitOrder, OpaqueAction};
    use crate::transaction::{Action, ActionMeta, SignatureDomain};
    use solana_pubkey::Pubkey;
    use std::sync::Arc;

    fn signable_hash_line(message: &str) -> &str {
        message
            .lines()
            .find(|line| line.starts_with("Signable-Hash: "))
            .expect("missing signable hash line")
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
        let first = canonical_message(SignatureDomain::Devnet, account, 42, actions.as_slice())
            .expect("build message");
        let second = canonical_message(SignatureDomain::Devnet, account, 42, actions.as_slice())
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
        let mainnet = canonical_message(SignatureDomain::Mainnet, account, 42, &actions)
            .expect("mainnet message");
        let testnet = canonical_message(SignatureDomain::Testnet, account, 42, &actions)
            .expect("testnet message");

        assert!(mainnet.contains("Network: mainnet"));
        assert!(testnet.contains("Network: testnet"));
        assert_ne!(signable_hash_line(&mainnet), signable_hash_line(&testnet));
    }

    #[test]
    fn message_contains_expected_fields() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::Faucet(Faucet {
            user: account,
            amount: None,
            meta: ActionMeta::default(),
        })];
        let message = canonical_message(SignatureDomain::Devnet, account, 42, actions.as_slice())
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
        let message = canonical_message(SignatureDomain::Devnet, account, 99, actions.as_slice())
            .expect("build message");
        assert!(message.contains("ETH-USD"));
        assert!(message.contains("Sell"));
        assert!(message.contains("3500.00000000"));
        assert!(message.contains("1.50000000"));
    }

    #[test]
    fn message_shows_builder_code_fields() {
        let account = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let actions = vec![Action::LimitOrder(LimitOrder {
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
        })];
        let message = canonical_message(SignatureDomain::Devnet, account, 99, actions.as_slice())
            .expect("build message");
        assert!(message.contains(&format!("builder_code_to={recipient}")));
        assert!(message.contains("builder_code_fee=5bps"));
        assert!(!message.contains("commission_to"));
        assert!(!message.contains("commission_fee"));
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
        let msg_one =
            canonical_message(SignatureDomain::Devnet, account, 42, actions_one.as_slice())
                .expect("one");
        let msg_two =
            canonical_message(SignatureDomain::Devnet, account, 42, actions_two.as_slice())
                .expect("two");
        assert_ne!(msg_one, msg_two);
        assert!(msg_one.contains("amount=1.00000000"));
        assert!(msg_two.contains("amount=1.00000000"));
        assert_ne!(
            signable_hash_line(msg_one.as_str()),
            signable_hash_line(msg_two.as_str())
        );
    }

    #[test]
    fn message_hashes_opaque_payload_preview() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::ConfigRisk(OpaqueAction {
            payload: vec![7; 128],
            meta: ActionMeta::default(),
        })];
        let message = canonical_message(SignatureDomain::Devnet, account, 42, actions.as_slice())
            .expect("build message");

        assert!(message.contains("ConfigRiskMatrix payload_len=128 payload_sha256="));
        assert!(!message.contains("payload=7"));
    }
}
