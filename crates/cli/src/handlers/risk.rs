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

    let config = parse_liquidator_config(&raw)?;

    eprintln!("Placing liquidator config update");
    let action = Action::UpdateLiquidatorConfig(config);
    submit_actions(api, submit, vec![action]).await
}

fn parse_liquidator_config(raw: &str) -> eyre::Result<LiqConfig> {
    let mut value: serde_json::Value =
        json5::from_str(raw).map_err(|e| eyre::eyre!("invalid liquidator config: {e}"))?;

    if let Some(serde_json::Value::Object(instruments)) = value.get_mut("instruments") {
        let instruments = std::mem::take(instruments)
            .into_iter()
            .map(|(symbol, value)| {
                let serde_json::Value::Object(mut config) = value else {
                    return Err(eyre::eyre!(
                        "invalid liquidator config: instrument '{symbol}' must be an object"
                    ));
                };
                config.insert("symbol".to_string(), serde_json::Value::String(symbol));
                Ok(serde_json::Value::Object(config))
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        value["instruments"] = serde_json::Value::Array(instruments);
    }

    serde_json::from_value(value).map_err(|e| eyre::eyre!("invalid liquidator config: {e}"))
}

#[cfg(test)]
mod tests {
    use super::parse_liquidator_config;

    const GLOBAL_FIELDS: &str = r#"
        "cross_exposure": 2500000.0,
        "scoring_skew": 0.5,
        "toxicity": 0.0,
        "urgency_size_fraction": 0.25,
        "sweep_sds": 2.0,
        "price_to_sweep": false
    "#;

    const INSTRUMENT_FIELDS: &str = r#"
        "apply_reserve": false,
        "dump_retry_secs": 60,
        "execution_mode": "normal",
        "max_adl_notional": 100000.0,
        "max_adl_percent": 100.0,
        "max_exposure": 1000000.0,
        "max_sweep_bps": 35.0,
        "reserve": 65.0,
        "rfactor": 0.25,
        "volume_min": 0.5,
        "volume_percent": 25.0,
        "volume_rampup": 0
    "#;

    #[test]
    fn parses_transaction_instrument_array() {
        let raw = format!(
            "{{{GLOBAL_FIELDS}, \"instruments\": [{{\"symbol\": \"BTC-USD\", {INSTRUMENT_FIELDS}}}]}}"
        );

        let config = parse_liquidator_config(&raw).unwrap();

        assert_eq!(config.instruments.len(), 1);
        assert_eq!(config.instruments[0].symbol, "BTC-USD");
        assert!(!config.instruments[0].apply_reserve);
    }

    #[test]
    fn converts_api_instrument_map() {
        let raw = format!(
            "{{{GLOBAL_FIELDS}, \"instruments\": {{\"BTC-USD\": {{{INSTRUMENT_FIELDS}}}}}, \
             \"owner\": \"ignored\", \"strategy_pubkey\": \"ignored\"}}"
        );

        let config = parse_liquidator_config(&raw).unwrap();

        assert_eq!(config.instruments.len(), 1);
        assert_eq!(config.instruments[0].symbol, "BTC-USD");
        assert_eq!(config.instruments[0].reserve, 65.0);
    }
}
