use std::path::Path;
use bulk_client::BulkHttpClient;
use bulk_client::msgs::liquidator::LiqConfig;
use bulk_client::msgs::risk::RiskConfigChange;
use bulk_client::transaction::Action;
use crate::commands::LiquidatorConfigArgs;
use crate::commands::risk::RiskConfigArgs;
use crate::common::submit::{submit_actions, SubmitOptions};

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

    let config: RiskConfigChange = json5::from_str(&raw)
        .map_err(|e| eyre::eyre!("invalid risk config: {e}"))?;

    eprintln!("Placing risk config update");
    let action = Action::UpdateRiskConfig(config);
    submit_actions(api, submit, vec![action]).await
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

    let config: LiqConfig = json5::from_str(&raw)
        .map_err(|e| eyre::eyre!("invalid liquidator config: {e}"))?;

    eprintln!("Placing liquidator config update");
    let action = Action::UpdateLiquidatorConfig(config);
    submit_actions(api, submit, vec![action]).await
}
