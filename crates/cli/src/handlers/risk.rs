use crate::commands::risk::{
    AccountPolicyArgs, FundingConfigArgs, LiquidatorConfigArgs, RiskConfigArgs, UserAdminArgs,
};
use crate::common::submit::{submit_actions, SubmitOptions};
use bulk_client::msgs::liquidator::LiqConfig;
use bulk_client::msgs::risk::RiskConfigChange;
use bulk_client::msgs::ConfigFunding;
use bulk_client::msgs::UpdateAccountPolicy;
use bulk_client::msgs::UserAdmin;
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;
use std::path::Path;

pub async fn handle_risk_config(
    api: &mut BulkHttpClient,
    args: RiskConfigArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|e| eyre::eyre!("failed to read '{}': {e}", args.json))?
    } else {
        args.json.clone()
    };

    let config: RiskConfigChange =
        json5::from_str(&raw).map_err(|e| eyre::eyre!("invalid risk config: {e}"))?;

    eprintln!("Placing risk config update");
    let action = Action::UpdateRiskConfig(config);
    submit_actions(api, submit, vec![action]).await
}

/// Submits an instrument funding configuration through the administrative multisig.
///
/// # Arguments
/// * `api` - HTTP client used to submit the proposal transaction.
/// * `args` - JSON text or a path containing the complete instrument funding config.
/// * `submit` - Preview and confirmation behavior for the submission.
///
/// # Returns
/// An error when the input cannot be read, parsed, or submitted.
pub async fn handle_funding_config(
    api: &mut BulkHttpClient,
    args: FundingConfigArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|e| eyre::eyre!("failed to read '{}': {e}", args.json))?
    } else {
        args.json.clone()
    };

    let config: ConfigFunding =
        json5::from_str(&raw).map_err(|e| eyre::eyre!("invalid funding config: {e}"))?;

    eprintln!("Placing funding config update for {}", config.symbol);
    submit_actions(api, submit, vec![Action::ConfigFunding(config)]).await
}

/// Submits an account funding policy update through the administrative multisig.
///
/// # Arguments
/// * `api` - HTTP client used to submit the proposal transaction.
/// * `args` - JSON text or a path containing the optional policy fields.
/// * `submit` - Preview and confirmation behavior for the submission.
///
/// # Returns
/// An error when the input cannot be read, parsed, or submitted.
pub async fn handle_account_policy(
    api: &mut BulkHttpClient,
    args: AccountPolicyArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|e| eyre::eyre!("failed to read '{}': {e}", args.json))?
    } else {
        args.json.clone()
    };

    let policy: UpdateAccountPolicy =
        json5::from_str(&raw).map_err(|e| eyre::eyre!("invalid account policy: {e}"))?;

    eprintln!("Placing account policy update");
    submit_actions(api, submit, vec![Action::UpdateAccountPolicy(policy)]).await
}

/// Submits account and optional global open-order limits through the admin multisig.
///
/// # Arguments
/// * `api` - HTTP client used to submit the proposal transaction.
/// * `args` - Target account, account override behavior, and optional global fallback.
/// * `submit` - Preview and confirmation behavior for the submission.
///
/// # Returns
/// An error when the action cannot be submitted.
pub async fn handle_user_admin(
    api: &mut BulkHttpClient,
    args: UserAdminArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let maxorders = if args.use_global {
        None
    } else {
        args.maxorders
    };
    eprintln!(
        "Updating open-order limits for {}: account={:?}, global={:?}",
        args.pubkey, maxorders, args.global_maxorders
    );
    submit_actions(
        api,
        submit,
        vec![Action::UserAdmin(UserAdmin {
            pubkey: args.pubkey,
            maxorders,
            global_maxorders: args.global_maxorders,
            meta: Default::default(),
        })],
    )
    .await
}

pub async fn handle_liquidator_config(
    api: &mut BulkHttpClient,
    args: LiquidatorConfigArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|e| eyre::eyre!("failed to read '{}': {e}", args.json))?
    } else {
        args.json.clone()
    };

    let config: LiqConfig =
        json5::from_str(&raw).map_err(|e| eyre::eyre!("invalid liquidator config: {e}"))?;

    eprintln!("Placing liquidator config update");
    let action = Action::UpdateLiquidatorConfig(config);
    submit_actions(api, submit, vec![action]).await
}
