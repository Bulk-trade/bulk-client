use bulk_client::msgs::{
    AddMarket, ConfigMakerRebateTier, MarketAction, MarketAdmin, Matrix, OpaqueAction, PricingAdmin,
};
use bulk_client::sdk::{time_epoch_ns, MktId, RiskMatrix, Security};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;
use std::path::Path;

use crate::commands::{
    AddMarketArgs, ConfigFeesArgs, ConfigMakerArgs, ConfigRiskArgs, ConfigSecurityArgs, CorrsArgs,
    FeePolicyUpdate, MarketAdminArgs, PricingAdminArgs,
};
use crate::common::submit::{submit_actions, SubmitOptions};

fn read_json5<T: serde::de::DeserializeOwned>(path: &str, label: &str) -> eyre::Result<T> {
    json5::from_str(
        &std::fs::read_to_string(path)
            .map_err(|error| eyre::eyre!("failed to read '{path}': {error}"))?,
    )
    .map_err(|error| eyre::eyre!("invalid {label} '{path}': {error}"))
}

fn read_json5_or_inline<T: serde::de::DeserializeOwned>(
    json_or_path: &str,
    label: &str,
) -> eyre::Result<T> {
    let raw = if Path::new(json_or_path).exists() {
        std::fs::read_to_string(json_or_path)
            .map_err(|error| eyre::eyre!("failed to read '{json_or_path}': {error}"))?
    } else {
        json_or_path.to_owned()
    };
    json5::from_str(&raw).map_err(|error| eyre::eyre!("invalid {label}: {error}"))
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

/// Replaces one coin's complete risk surface through the administrative multisig.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Coin name and path to its risk-surface CSV.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when the coin is invalid, the CSV cannot be parsed, or submission fails.
pub async fn handle_config_risk(
    api: &mut BulkHttpClient,
    args: ConfigRiskArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    Security::initialize();
    let instrument = MktId::from_str(&args.coin)
        .ok_or_else(|| eyre::eyre!("invalid risk-surface coin '{}'", args.coin))?;
    let matrix = RiskMatrix::from_csv(time_epoch_ns(), instrument, &args.csv)
        .map_err(|error| eyre::eyre!("invalid risk-surface CSV '{}': {error}", args.csv))?;
    let payload = bincode::serialize(&matrix)
        .map_err(|error| eyre::eyre!("failed to serialize risk surface: {error}"))?;

    eprintln!("Placing risk-surface replacement for {}", args.coin);
    submit_actions(
        api,
        submit,
        vec![Action::ConfigRisk(OpaqueAction {
            payload,
            meta: Default::default(),
        })],
    )
    .await
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

/// Submits a complete security definition through the administrative multisig.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Inline JSON/JSON5 or a path containing one security definition.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when the definition cannot be read, parsed, serialized, or submitted.
pub async fn handle_config_security(
    api: &mut BulkHttpClient,
    args: ConfigSecurityArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let raw = if Path::new(&args.json).exists() {
        std::fs::read_to_string(&args.json)
            .map_err(|error| eyre::eyre!("failed to read '{}': {error}", args.json))?
    } else {
        args.json
    };
    let security: Security =
        json5::from_str(&raw).map_err(|error| eyre::eyre!("invalid security config: {error}"))?;
    let payload = bincode::serialize(&security)
        .map_err(|error| eyre::eyre!("failed to serialize security config: {error}"))?;

    eprintln!("Placing security configuration update");
    submit_actions(
        api,
        submit,
        vec![Action::ConfigSecurity(OpaqueAction {
            payload,
            meta: Default::default(),
        })],
    )
    .await
}

/// Submits a fee-policy update through the administrative multisig.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Inline JSON/JSON5 or a path containing a fee-policy update.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when the update cannot be parsed, encoded, or submitted.
pub async fn handle_config_fees(
    api: &mut BulkHttpClient,
    args: ConfigFeesArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    const FEE_POLICY_PAYLOAD_MAGIC: &[u8; 4] = b"FEE2";
    const FEE_POLICY_PAYLOAD_VERSION: u8 = 1;

    let update: FeePolicyUpdate = read_json5_or_inline(&args.json, "fee policy update")?;
    let encoded = bincode::serialize(&update)
        .map_err(|error| eyre::eyre!("failed to encode fee policy update: {error}"))?;
    let mut payload = Vec::with_capacity(FEE_POLICY_PAYLOAD_MAGIC.len() + 1 + encoded.len());
    payload.extend_from_slice(FEE_POLICY_PAYLOAD_MAGIC);
    payload.push(FEE_POLICY_PAYLOAD_VERSION);
    payload.extend_from_slice(&encoded);

    eprintln!("Placing fee policy configuration update");
    submit_actions(
        api,
        submit,
        vec![Action::ConfigFeePolicy(OpaqueAction {
            payload,
            meta: Default::default(),
        })],
    )
    .await
}

/// Submits a maker rebate tier override through the administrative multisig.
///
/// # Arguments
/// * `api` - Authenticated Bulk HTTP client.
/// * `args` - Inline JSON/JSON5 or a path containing the maker override.
/// * `submit` - Transaction preview and confirmation options.
///
/// # Returns
/// An error when the override cannot be parsed or submitted.
pub async fn handle_config_maker(
    api: &mut BulkHttpClient,
    args: ConfigMakerArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let mut config: ConfigMakerRebateTier =
        read_json5_or_inline(&args.json, "maker rebate tier override")?;
    config.meta = Default::default();

    eprintln!(
        "Configuring maker rebate tier for {} on {}",
        config.maker, config.instrument
    );
    submit_actions(api, submit, vec![Action::ConfigMakerRebateTier(config)]).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialized_security_registry_resolves_risk_matrix_coin() {
        Security::initialize();

        assert!(MktId::from_str("BTC").is_some());
    }

    #[test]
    fn security_payload_roundtrips_through_executor_decoder() {
        let security: Security = json5::from_str(
            r#"{
                type: "Currency",
                name: "BTC",
                dollarQuoted: true,
                dollarEquivalent: false,
                sid: 100,
                pyth: 1,
                aliases: ["WBTC"],
                decimals: 8,
                address: {
                    Solana: "3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh"
                }
            }"#,
        )
        .expect("parse BTC security");

        let payload = bincode::serialize(&security).expect("serialize security payload");
        let decoded: Security =
            bincode::deserialize(&payload).expect("executor-compatible security payload");

        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(security).unwrap()
        );
    }

    #[test]
    fn fee_policy_payload_uses_versioned_executor_envelope() {
        let update: FeePolicyUpdate = json5::from_str(
            r#"{
                effectiveSlot: null,
                clearScheduled: false,
                disable: false,
                policy: {
                    windowDays: 14,
                    tiers: [{ thresholdVolume: 0, makerBps: 2, takerBps: 5 }]
                }
            }"#,
        )
        .expect("parse fee policy update");
        let encoded = bincode::serialize(&update).expect("encode fee policy update");

        let mut payload = b"FEE2\x01".to_vec();
        payload.extend_from_slice(&encoded);

        assert_eq!(&payload[..5], b"FEE2\x01");
        let decoded: FeePolicyUpdate =
            bincode::deserialize(&payload[5..]).expect("decode fee policy update");
        assert_eq!(decoded.policy.unwrap().tiers.len(), 1);
    }
}
