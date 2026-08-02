use bulk_client::msgs::{AddMarket, MarketAction, MarketAdmin, Matrix, PricingAdmin};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;

use crate::commands::{AddMarketArgs, CorrsArgs, MarketAdminArgs, PricingAdminArgs};
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

/// Applies an administrative market-state transition.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Market symbol, transition, and optional close price.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when arguments are invalid or transaction submission fails.
pub async fn handle_market_admin(
    api: &mut BulkHttpClient,
    args: MarketAdminArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let action = MarketAction::from(args.action);
    if args.price.is_some() && action != MarketAction::Close {
        return Err(eyre::eyre!("--price is valid only for the close action"));
    }
    if args
        .price
        .is_some_and(|price| !price.is_finite() || price <= 0.0)
    {
        return Err(eyre::eyre!("close price must be finite and positive"));
    }

    eprintln!("Applying {:?} to market {}", action, args.symbol);
    submit_actions(
        api,
        submit,
        vec![Action::MarketAdmin(MarketAdmin {
            symbol: args.symbol.into(),
            action,
            price: args.price,
            meta: Default::default(),
        })],
    )
    .await
}

/// Configures the accepted oracle source for an instrument.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Oracle instrument and accepted source.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when transaction submission fails.
pub async fn handle_pricing_admin(
    api: &mut BulkHttpClient,
    args: PricingAdminArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let source = args.source.into();
    eprintln!(
        "Setting pricing source for {} to {:?}",
        args.instrument, source
    );
    submit_actions(
        api,
        submit,
        vec![Action::PricingAdmin(PricingAdmin {
            instrument: args.instrument.into(),
            source,
            meta: Default::default(),
        })],
    )
    .await
}
