use bulk_client::msgs::{AddMarket, Matrix};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;

use crate::commands::{AddMarketArgs, CorrsArgs};
use crate::common::submit::{submit_actions, SubmitOptions};

fn read_json5<T: serde::de::DeserializeOwned>(path: &str, label: &str) -> eyre::Result<T> {
    json5::from_str(
        &std::fs::read_to_string(path)
            .map_err(|error| eyre::eyre!("failed to read '{path}': {error}"))?,
    )
    .map_err(|error| eyre::eyre!("invalid {label} '{path}': {error}"))
}

pub async fn handle_corrs(
    api: &mut BulkHttpClient,
    args: CorrsArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let value = read_json5::<serde_json::Value>(&args.json, "correlation matrix")?;
    let matrix = serde_json::from_value::<Matrix>(value.get("matrix").cloned().unwrap_or(value))
        .map_err(|error| eyre::eyre!("invalid correlation matrix '{}': {error}", args.json))?;

    eprintln!(
        "Placing correlation matrix ({} markets)",
        matrix.index.len()
    );
    submit_actions(api, submit, vec![Action::Corrs(matrix)]).await
}

pub async fn handle_add_market(
    api: &mut BulkHttpClient,
    args: AddMarketArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    eprintln!("Adding market {}", args.symbol);
    submit_actions(
        api,
        submit,
        vec![Action::AddMarket(AddMarket {
            symbol: args.symbol.into(),
            meta: Default::default(),
        })],
    )
    .await
}
