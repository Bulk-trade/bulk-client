use std::path::Path;
use bulk_client::BulkHttpClient;
use bulk_client::msgs::risk::RiskConfigChange;
use bulk_client::transaction::{Action};
use crate::commands::risk::RiskConfigArgs;

pub async fn handle_risk_config(
    api: &mut BulkHttpClient,
    args: RiskConfigArgs,
) -> eyre::Result<()> {
    // Resolve the raw JSON5 text: treat `json` as a file path if the path
    // exists on disk, otherwise use the string directly as inline JSON5.
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|e| eyre::eyre!("failed to read '{}': {e}", args.json))?
    } else {
        args.json.clone()
    };

    let config: RiskConfigChange = json5::from_str(&raw)
        .map_err(|e| eyre::eyre!("invalid risk config: {e}"))?;

    println!("Placing risk configs {:?}", config);

    let action = Action::UpdateRiskConfig(config);

    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}\n", results);
    Ok(())
}
