use std::fmt::Write as _;
use crate::msgs::conditional::{OnFill, Range, StopOrTP, Trailing, Trigger};
use crate::msgs::multisig::{CreateMultisig, UpdateMultisigPolicy};
use crate::msgs::UpdateUserSettings;
use crate::transaction::Action;
use solana_pubkey::Pubkey;

#[derive(Clone, Copy, Debug, Default)]
pub struct ClearSignMessageOptions {
    pub include_signable_schema: bool,
}

pub fn canonical_message(account: Pubkey, nonce: u64, actions: &[Action]) -> eyre::Result<String> {
    canonical_message_with_options(account, nonce, actions, ClearSignMessageOptions::default())
}

pub fn canonical_message_with_options(
    account: Pubkey,
    nonce: u64,
    actions: &[Action],
    options: ClearSignMessageOptions,
) -> eyre::Result<String> {
    let signable = signable_bytes(account, nonce, actions)?;
    let mut message = String::with_capacity(256 + actions.len().saturating_mul(96));
    let _ = writeln!(message, "Bulk Exchange Transaction");
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
            "Signable-Schema: bincode(actions)||nonce_le_u64||account_bytes"
        );
    }
    for (index, action) in actions.iter().enumerate() {
        let _ = writeln!(message, "[{}] {}", index, action_line(action));
    }
    Ok(message)
}

fn signable_bytes(account: Pubkey, nonce: u64, actions: &[Action]) -> eyre::Result<Vec<u8>> {
    let mut signable = bincode::serialize(actions)?;
    signable.extend_from_slice(&nonce.to_le_bytes());
    signable.extend_from_slice(account.as_ref());
    Ok(signable)
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
        Action::WhitelistFaucet(action) => {
            format!(
                "WhitelistFaucet target={} whitelist={}",
                action.target, action.whitelist
            )
        }
        Action::AddMarket(action) => format!("AddMarket {}", action.symbol),
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
        Action::UpdateRiskConfig(action) => format!(
            "UpdateRiskConfig max_loss={:.8} eloss_floor={:.8} max_pliq={:.8} margin_buffer={:.8}",
            action.max_loss, action.eloss_floor, action.max_pliq, action.margin_buffer
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
        "OnFill parent={} nested={}",
        action.parent_seqno,
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
    use crate::msgs::{Faucet, LimitOrder};
    use crate::transaction::{Action, ActionMeta};
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
            meta: ActionMeta::default(),
        })];
        let first = canonical_message(account, 42, actions.as_slice()).expect("build message");
        let second = canonical_message(account, 42, actions.as_slice()).expect("build message");
        assert_eq!(first, second);
    }

    #[test]
    fn message_contains_expected_fields() {
        let account = Pubkey::new_unique();
        let actions = vec![Action::Faucet(Faucet {
            user: account,
            amount: None,
            meta: ActionMeta::default(),
        })];
        let message = canonical_message(account, 42, actions.as_slice()).expect("build message");
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
            meta: ActionMeta::default(),
        })];
        let message = canonical_message(account, 99, actions.as_slice()).expect("build message");
        assert!(message.contains("ETH-USD"));
        assert!(message.contains("Sell"));
        assert!(message.contains("3500.00000000"));
        assert!(message.contains("1.50000000"));
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
        let msg_one = canonical_message(account, 42, actions_one.as_slice()).expect("one");
        let msg_two = canonical_message(account, 42, actions_two.as_slice()).expect("two");
        assert_ne!(msg_one, msg_two);
        assert!(msg_one.contains("amount=1.00000000"));
        assert!(msg_two.contains("amount=1.00000000"));
        assert_ne!(
            signable_hash_line(msg_one.as_str()),
            signable_hash_line(msg_two.as_str())
        );
    }
}
