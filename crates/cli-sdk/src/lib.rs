use bulk_client::common::WireMktId;
use bulk_client::msgs::{AddMarket, ConfigFunding, OpaqueAction};
use bulk_client::transaction::Action;
use bulk_sdk_core::data::models::{BarModelConfig, TickModelConfig};
use bulk_sdk_core::markets::{with_security_context, MktId};
use bulk_sdk_core::models::margin::RiskMatrix;
use bulk_sdk_core::models::signals::bulk_funding1::BulkFunding1;
use bulk_sdk_core::models::signals::{WrappedBarFunction1D, WrappedTickFunctionMD};
use bulk_sdk_core::securities::{Security, SecurityMap};
use bulk_sdk_core::time_epoch_ns;
use std::path::{Path, PathBuf};

/// Converts a native SDK market identifier into its public wire representation.
///
/// # Arguments
/// * `market_id` - Native SDK identifier to convert.
pub fn wire_mktid(market_id: MktId) -> WireMktId {
    WireMktId(market_id.uuid)
}

/// Builds a risk-matrix action using native SDK parsing and validation.
///
/// # Arguments
/// * `coin` - Registered SDK security name.
/// * `csv` - Path to the risk-surface CSV.
///
/// # Returns
/// An error when the security is unknown, CSV is invalid, or serialization fails.
pub fn config_risk_action(coin: &str, csv: &str) -> eyre::Result<Action> {
    let securities = SecurityMap::from_configs().map_err(|error| eyre::eyre!("{error:#}"))?;
    let instrument = securities
        .mktid_for(coin)
        .ok_or_else(|| eyre::eyre!("invalid risk-surface coin '{coin}'"))?;
    let matrix = RiskMatrix::from_csv(time_epoch_ns(), instrument, csv)
        .map_err(|error| eyre::eyre!("invalid risk-surface CSV '{csv}': {error}"))?;
    opaque_action(matrix, Action::ConfigRisk, "risk surface")
}

/// Builds security-configuration actions for one administrative proposal.
///
/// * Accepts one definition or an array and preserves the input order.
/// * Validates every definition before returning any actions.
///
/// # Arguments
/// * `json` - Inline JSON/JSON5 containing a security definition or an array of definitions.
///
/// # Returns
/// An error when parsing or serialization fails, or the array is empty.
pub fn config_security_actions(json: &str) -> eyre::Result<Vec<Action>> {
    let value: serde_json::Value =
        json5::from_str(json).map_err(|error| eyre::eyre!("invalid security config: {error}"))?;
    let definitions = match value {
        serde_json::Value::Array(definitions) => definitions,
        definition => vec![definition],
    };
    if definitions.is_empty() {
        eyre::bail!("security config array must not be empty");
    }
    definitions
        .into_iter()
        .enumerate()
        .map(|(index, definition)| {
            // SDK order types deserialize through borrowed strings, which an owned
            // Value deserializer cannot supply. Parse from text to retain borrowing.
            let json = serde_json::to_string(&definition)?;
            let security: Security = serde_json::from_str(&json).map_err(|error| {
                eyre::eyre!("invalid security config at index {index}: {error}")
            })?;
            opaque_action(security, Action::ConfigSecurity, "security config")
        })
        .collect()
}

/// Builds a security-configuration action using the native SDK security type.
///
/// # Arguments
/// * `json` - Inline JSON/JSON5 containing one complete security definition.
///
/// # Returns
/// An error when parsing or serialization fails.
pub fn config_security_action(json: &str) -> eyre::Result<Action> {
    let security: Security =
        json5::from_str(json).map_err(|error| eyre::eyre!("invalid security config: {error}"))?;
    opaque_action(security, Action::ConfigSecurity, "security config")
}

/// Builds the complete ordered action list for a new market, using its config namespace.
///
/// * Reads the currency, perpetual, tier references, and executor funding configuration.
/// * Loads securities from the supplied configuration tree, including the new market.
/// * Validates and serializes securities, reference models, and the risk surface.
/// * Selects and validates executor funding parameters before appending market creation.
///
/// # Arguments
/// * `config_dir` - Root of the securities, models, and risk configuration tree.
/// * `executor_config` - Executor JSON5 file supplying funding defaults and overrides.
/// * `symbol` - Underlier currency name, such as PAPER.
/// * `tier` - Reference tier: low, medium, or high.
/// * `skip_currency` - Whether the currency already exists on the exchange.
///
/// # Returns
/// An error when a definition, model, symbol, or risk surface is invalid.
pub fn create_market_actions(
    config_dir: &Path,
    executor_config: &Path,
    symbol: &str,
    tier: &str,
    skip_currency: bool,
) -> eyre::Result<Vec<Action>> {
    let plan = read_market_plan(
        config_dir,
        executor_config,
        symbol,
        Some(tier),
        skip_currency,
    )?;
    build_market_actions(plan)
}

/// Builds a market proposal from symbol-specific models and risk configuration.
///
/// * Reads the currency, perpetual, named models, risk surface, and funding configuration.
/// * Validates the configuration and builds the ordered market actions.
///
/// # Arguments
/// * `config_dir` - Root of the securities, models, and risk configuration tree.
/// * `executor_config` - Executor JSON5 file supplying funding defaults and overrides.
/// * `symbol` - Underlier currency name used for model and risk filenames.
/// * `skip_currency` - Whether the currency already exists on the exchange.
///
/// # Returns
/// An error when a required file is missing or its configuration is invalid.
pub fn create_named_market_actions(
    config_dir: &Path,
    executor_config: &Path,
    symbol: &str,
    skip_currency: bool,
) -> eyre::Result<Vec<Action>> {
    let plan = read_market_plan(config_dir, executor_config, symbol, None, skip_currency)?;
    build_market_actions(plan)
}

/// Builds a liquidator update, replacing the pair policy with its local configuration.
///
/// * Uses the local pair override or local wildcard, falling back to the live wildcard.
/// * Preserves live global settings and every unrelated instrument policy.
///
/// # Arguments
/// * `config` - Current JSON returned by the liquidator configuration API.
/// * `executor_config` - Local executor JSON5 supplying the new instrument policy.
/// * `symbol` - Traded pair name, such as PAPER-USD.
///
/// # Returns
/// A full configuration update, or an error for invalid configuration or a missing policy.
pub fn market_liquidator_action(
    mut config: serde_json::Value,
    executor_config: &str,
    symbol: &str,
) -> eyre::Result<Action> {
    if let Some(serde_json::Value::Object(instruments)) = config.get_mut("instruments") {
        let entries = std::mem::take(instruments)
            .into_iter()
            .map(|(name, mut entry)| {
                let policy = entry.as_object_mut().ok_or_else(|| {
                    eyre::eyre!("liquidator instrument '{name}' must be an object")
                })?;
                policy.insert("symbol".into(), serde_json::Value::String(name));
                Ok(entry)
            })
            .collect::<eyre::Result<Vec<_>>>()?;
        config["instruments"] = serde_json::Value::Array(entries);
    }
    let mut config: bulk_client::msgs::liquidator::LiqConfig = serde_json::from_value(config)?;
    let local: serde_json::Value = json5::from_str(executor_config)?;
    let policies = local
        .get("liquidator")
        .and_then(|value| value.get("instruments"));
    let selected = policies.and_then(|value| value.get(symbol).or_else(|| value.get("*")));
    let mut policy = if let Some(selected) = selected {
        let mut selected = selected.clone();
        selected
            .as_object_mut()
            .ok_or_else(|| eyre::eyre!("local liquidator policy for {symbol} must be an object"))?
            .insert("symbol".into(), serde_json::Value::String(symbol.into()));
        serde_json::from_value(selected)?
    } else {
        let mut matches = config
            .instruments
            .iter()
            .filter(|entry| entry.symbol == "*");
        let policy = matches
            .next()
            .ok_or_else(|| eyre::eyre!("liquidator configuration has no wildcard '*' policy"))?
            .clone();
        if matches.next().is_some() {
            eyre::bail!("liquidator configuration has multiple wildcard policies");
        }
        policy
    };
    config.instruments.retain(|entry| entry.symbol != symbol);
    policy.symbol = symbol.into();
    config.instruments.push(policy);
    Ok(Action::UpdateLiquidatorConfig(config))
}

// ───── Configuration Discovery ───────────────────────────────────────────────────────────────────

fn read_market_plan(
    config_dir: &Path,
    executor_config: &Path,
    symbol: &str,
    tier: Option<&str>,
    skip_currency: bool,
) -> eyre::Result<MarketPlan> {
    if symbol.is_empty()
        || !symbol.as_bytes()[0].is_ascii_uppercase()
        || !symbol
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
    {
        eyre::bail!("symbol must be an uppercase currency name, e.g. PAPER");
    }
    if tier.is_some_and(|tier| !matches!(tier, "low" | "medium" | "high")) {
        eyre::bail!("tier must be low, medium, or high");
    }
    let securities_dir = config_dir.join("securities");
    let currency =
        read_security_definition(&securities_dir.join("Currencies.json"), symbol, "Currency")?;
    let pair = format!("{symbol}-USD");
    let perpetual =
        read_security_definition(&securities_dir.join("Perpetuals.json"), &pair, "Perpetual")?;
    if perpetual.get("pccy").and_then(serde_json::Value::as_str) != Some(symbol)
        || perpetual.get("qccy").and_then(serde_json::Value::as_str) != Some("USD")
    {
        eyre::bail!("perpetual must have the selected underlier and USD quote currency");
    }
    let mut securities = Vec::new();
    if !skip_currency {
        securities.push(currency);
    }
    securities.push(perpetual);
    let risk_csv = match tier {
        Some(tier) => config_dir.join("risk/ref").join(format!("{tier}.csv")),
        None => config_dir.join("risk").join(format!("{symbol}.csv")),
    };
    if !risk_csv.is_file() {
        eyre::bail!("missing risk configuration: {}", risk_csv.display());
    }
    Ok(MarketPlan {
        symbol: symbol.into(),
        securities_dir,
        securities,
        volatility: read_market_model(config_dir, "volatility", tier, symbol)?,
        fairprice: read_market_model(config_dir, "fairprice", tier, symbol)?,
        regime: read_market_model(config_dir, "regime", tier, symbol)?,
        risk_csv,
        executor_config: read_config(executor_config)?,
    })
}

fn read_security_definition(
    path: &Path,
    name: &str,
    kind: &str,
) -> eyre::Result<serde_json::Value> {
    let definitions: Vec<serde_json::Value> = serde_json::from_str(&read_config(path)?)?;
    select_security_definition(definitions, name, kind)
}

fn select_security_definition(
    definitions: Vec<serde_json::Value>,
    name: &str,
    kind: &str,
) -> eyre::Result<serde_json::Value> {
    let mut matches = definitions
        .into_iter()
        .filter(|item| item.get("name").and_then(serde_json::Value::as_str) == Some(name));
    let definition = matches
        .next()
        .ok_or_else(|| eyre::eyre!("missing {kind} named {name}"))?;
    if matches.next().is_some()
        || definition.get("type").and_then(serde_json::Value::as_str) != Some(kind)
    {
        eyre::bail!("expected exactly one {kind} named {name}");
    }
    Ok(definition)
}

/// Reads a named model directly or expands a BIP1 tier reference.
fn read_market_model(
    config_dir: &Path,
    kind: &str,
    tier: Option<&str>,
    symbol: &str,
) -> eyre::Result<String> {
    match tier {
        Some(tier) => read_reference(config_dir, kind, tier, symbol),
        None => read_config(
            &config_dir
                .join("models")
                .join(kind)
                .join(format!("{symbol}.json5")),
        ),
    }
}

fn read_reference(config_dir: &Path, kind: &str, tier: &str, symbol: &str) -> eyre::Result<String> {
    let directory = config_dir.join("models").join(kind).join("ref");
    let candidates = [
        directory.join(format!("{tier}.json5")),
        directory.join(format!("BIP1_{}.json5", tier.to_uppercase())),
    ];
    let matches: Vec<_> = candidates.iter().filter(|path| path.is_file()).collect();
    if matches.len() != 1 {
        eyre::bail!("expected exactly one {kind} reference: {candidates:?}");
    }
    Ok(read_config(matches[0])?.replace("SYMBOL", symbol))
}

fn read_config(path: &Path) -> eyre::Result<String> {
    std::fs::read_to_string(path)
        .map_err(|error| eyre::eyre!("failed to read {}: {error}", path.display()))
}

// ───── Action Construction ────────────────────────────────────────────────────────────────────────

fn build_market_actions(plan: MarketPlan) -> eyre::Result<Vec<Action>> {
    let securities = SecurityMap::from_config_dir(&plan.securities_dir)
        .map_err(|error| eyre::eyre!("{error:#}"))?;
    let instrument = securities
        .mktid_for(&plan.symbol)
        .ok_or_else(|| eyre::eyre!("unknown currency {}", plan.symbol))?;
    let mut actions = Vec::new();
    for definition in plan.securities {
        let security: Security = serde_json::from_str(&serde_json::to_string(&definition)?)?;
        security
            .validate(None)
            .map_err(|error| eyre::eyre!("{error:#}"))?;
        actions.push(opaque_action(security, Action::ConfigSecurity, "security")?);
    }
    with_security_context(&securities, || -> eyre::Result<()> {
        let stamp = time_epoch_ns();
        let mut vol: WrappedBarFunction1D = json5::from_str(&plan.volatility)?;
        vol.rehydrate(&securities)
            .map_err(|error| eyre::eyre!("{error:#}"))?;
        vol.validate().map_err(|error| eyre::eyre!("{error:#}"))?;
        actions.push(opaque_action(
            BarModelConfig {
                stamp,
                instrument,
                signal: vol,
            },
            Action::ConfigVolatility,
            "volatility",
        )?);
        let mut fair: WrappedTickFunctionMD = json5::from_str(&plan.fairprice)?;
        fair.rehydrate(&securities)
            .map_err(|error| eyre::eyre!("{error:#}"))?;
        fair.validate().map_err(|error| eyre::eyre!("{error:#}"))?;
        actions.push(opaque_action(
            TickModelConfig {
                stamp,
                instrument,
                signal: fair,
            },
            Action::ConfigFairPrice,
            "fair price",
        )?);
        let mut regime: WrappedBarFunction1D = json5::from_str(&plan.regime)?;
        regime
            .rehydrate(&securities)
            .map_err(|error| eyre::eyre!("{error:#}"))?;
        regime
            .validate()
            .map_err(|error| eyre::eyre!("{error:#}"))?;
        actions.push(opaque_action(
            BarModelConfig {
                stamp,
                instrument,
                signal: regime,
            },
            Action::ConfigRegime,
            "regime",
        )?);
        Ok(())
    })?;
    let matrix = RiskMatrix::from_csv(
        time_epoch_ns(),
        instrument,
        plan.risk_csv
            .to_str()
            .ok_or_else(|| eyre::eyre!("risk path must be UTF-8"))?,
    )
    .map_err(|error| eyre::eyre!("{error:#}"))?;
    actions.push(opaque_action(matrix, Action::ConfigRisk, "risk matrix")?);
    let symbol = format!("{}-USD", plan.symbol);
    let funding = funding_config_for(&plan.executor_config, &symbol)?;
    let market_id = securities
        .mktid_for(&symbol)
        .ok_or_else(|| eyre::eyre!("unknown perpetual {symbol}"))?;
    BulkFunding1::new(
        &securities,
        market_id,
        funding.sample_period,
        funding.mean_window,
        funding.notional,
        funding.rate,
        funding.deviation_cap,
        funding.funding_cap,
        funding.premium_horizon,
    )
    .map_err(|error| eyre::eyre!("invalid funding for {symbol}: {error:#}"))?;
    actions.push(Action::ConfigFunding(funding));
    actions.push(Action::AddMarket(AddMarket {
        symbol: format!("{}-USD", plan.symbol).into(),
        meta: Default::default(),
    }));
    Ok(actions)
}

struct MarketPlan {
    symbol: String,
    securities_dir: PathBuf,
    securities: Vec<serde_json::Value>,
    volatility: String,
    fairprice: String,
    regime: String,
    risk_csv: PathBuf,
    executor_config: String,
}

/// Selects the complete pair override, or the wildcard funding defaults, from executor JSON5.
fn funding_config_for(executor_config: &str, symbol: &str) -> eyre::Result<ConfigFunding> {
    let executor: serde_json::Value = json5::from_str(executor_config)?;
    let configs = executor
        .get("funding")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| eyre::eyre!("executor config must contain a funding object"))?;
    let selected = configs
        .get(symbol)
        .or_else(|| configs.get("*"))
        .ok_or_else(|| eyre::eyre!("no funding override for {symbol} or wildcard default"))?;
    let parameters = selected
        .as_object()
        .ok_or_else(|| eyre::eyre!("funding for {symbol} must be an object"))?;
    let mut wire = serde_json::Map::new();
    wire.insert("symbol".into(), serde_json::Value::String(symbol.into()));
    for (source, target) in [
        ("rate", "rate"),
        ("deviation_cap", "deviationCap"),
        ("funding_cap", "fundingCap"),
        ("premium_horizon", "premiumHorizon"),
        ("notional", "notional"),
        ("sample_period", "samplePeriod"),
        ("mean_window", "meanWindow"),
    ] {
        let value = parameters
            .get(source)
            .ok_or_else(|| eyre::eyre!("funding for {symbol} is missing {source}"))?;
        wire.insert(target.into(), value.clone());
    }
    Ok(serde_json::from_value(serde_json::Value::Object(wire))?)
}

fn opaque_action<T>(
    value: T,
    wrap: impl FnOnce(OpaqueAction) -> Action,
    label: &str,
) -> eyre::Result<Action>
where
    T: serde::Serialize,
{
    let payload = bincode::serialize(&value)
        .map_err(|error| eyre::eyre!("failed to serialize {label}: {error}"))?;
    Ok(wrap(OpaqueAction {
        payload,
        meta: Default::default(),
    }))
}

// ───── Regression Tests ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_config_accepts_single_and_array_in_order() {
        let first = r#"{type:'Currency',name:'SPACEX',pyth:3314,dollarQuoted:true,
            dollarEquivalent:false,priceBound:50.0,sid:155,decimals:8}"#;
        let second = first.replace("SPACEX", "SNDK").replace("155", "156");
        let array = format!("[// Preserve JSON5 support\n{first},{second},]");
        for (input, expected) in [
            (first.to_owned(), vec![first]),
            (array, vec![first, &second]),
        ] {
            let actions = config_security_actions(&input).unwrap();
            assert_eq!(actions.len(), expected.len());
            for (action, definition) in actions.into_iter().zip(expected) {
                let Action::ConfigSecurity(actual) = action else {
                    panic!("expected ConfigSecurity");
                };
                let Action::ConfigSecurity(expected) = config_security_action(definition).unwrap()
                else {
                    panic!("expected ConfigSecurity");
                };
                assert_eq!(actual.payload, expected.payload);
            }
        }
        let error = config_security_actions(&format!("[{first},{{type:'Invalid'}}]")).unwrap_err();
        assert!(error.to_string().contains("index 1"));
    }

    #[test]
    fn security_config_accepts_perpetual_order_types_in_mixed_batch() {
        let currency = r#"{"type":"Currency","name":"SPACEX","pyth":3314,"dollarQuoted":true,"dollarEquivalent":false,"priceBound":50.0,"sid":155,"decimals":8}"#;
        let perpetual = r#"{"type":"Perpetual","name":"SPACEX-USD","aliases":["FXP-SPACEX-USD-BULK"],"sid":28,"decimals":8,"venue":"Bulk","marginMode":"Cross","pricePrecision":3,"sizePrecision":4,"minNotional":50.0,"leverage":{"min":2.0,"max":10.0,"notional_0":100000.0,"notional_half":300000.0},"maxDepth":5000.0,"max_impact":400,"slippage":100,"stop_slippage":200,"pccy":"SPACEX","qccy":"USD","funding":"1hr","ordertypes":["Limit","Market"],"tif":["GTC","IOC"]}"#;
        for input in [perpetual.to_owned(), format!("[{currency},{perpetual}]")] {
            let actions = config_security_actions(&input).unwrap();
            let Action::ConfigSecurity(action) = actions.last().unwrap() else {
                panic!("expected ConfigSecurity");
            };
            let expected: Security = serde_json::from_str(perpetual).unwrap();
            assert_eq!(action.payload, bincode::serialize(&expected).unwrap());
            assert_eq!(actions.len(), if input.starts_with('[') { 2 } else { 1 });
        }
    }

    #[test]
    fn security_config_rejects_empty_arrays_and_invalid_input() {
        for input in ["[]", "null", "42", "{}", "[[]]", "[", "invalid"] {
            assert!(config_security_actions(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn liquidator_copies_wildcard_and_preserves_current_policies() {
        use bulk_client::msgs::liquidator::{ExecutionMode, LiqConfig, LiqConfigByInstrument};
        let wildcard = LiqConfigByInstrument {
            symbol: "*".into(),
            execution_mode: ExecutionMode::Paused,
            max_exposure: 12345.0,
            apply_reserve: true,
            ..Default::default()
        };
        let existing = LiqConfigByInstrument {
            symbol: "BTC-USD".into(),
            max_exposure: 999.0,
            ..wildcard.clone()
        };
        let config = LiqConfig {
            cross_exposure: 45678.0,
            instruments: vec![wildcard.clone(), existing.clone()],
            ..Default::default()
        };
        let array = serde_json::to_value(&config).unwrap();
        let mut object = array.clone();
        object["instruments"] = serde_json::json!({
            "*": wildcard, "BTC-USD": existing
        });
        for raw in [array.clone(), object] {
            let Action::UpdateLiquidatorConfig(updated) =
                market_liquidator_action(raw, "{}", "FOO-USD").unwrap()
            else {
                panic!("wrong action")
            };
            assert_eq!(updated.cross_exposure, config.cross_exposure);
            assert_eq!(updated.instruments.len(), 3);
            let new = updated.instruments.last().unwrap();
            assert_eq!(new.symbol, "FOO-USD");
            let mut copied = serde_json::to_value(new).unwrap();
            copied["symbol"] = serde_json::json!("*");
            assert_eq!(copied, serde_json::to_value(&wildcard).unwrap());
            for policy in &config.instruments {
                let preserved = updated
                    .instruments
                    .iter()
                    .find(|x| x.symbol == policy.symbol)
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(preserved).unwrap(),
                    serde_json::to_value(policy).unwrap()
                );
            }
        }
        let mut pair_policy = existing.clone();
        pair_policy.max_exposure = 321.0;
        pair_policy.volume_min = 143.0;
        let mut default_policy = wildcard.clone();
        default_policy.max_exposure = 654.0;
        let local = serde_json::json!({"liquidator": {"instruments": {
            "BTC-USD": pair_policy,
            "*": default_policy
        }}})
        .to_string();
        for target in ["BTC-USD", "FOO-USD"] {
            let Action::UpdateLiquidatorConfig(updated) =
                market_liquidator_action(array.clone(), &local, target).unwrap()
            else {
                panic!("wrong action")
            };
            let policies: Vec<_> = updated
                .instruments
                .iter()
                .filter(|p| p.symbol == target)
                .collect();
            assert_eq!(policies.len(), 1);
            assert_eq!(
                policies[0].max_exposure,
                if target == "BTC-USD" { 321.0 } else { 654.0 }
            );
            if target == "BTC-USD" {
                assert_eq!(policies[0].volume_min, 143.0);
            }
            assert_eq!(updated.cross_exposure, config.cross_exposure);
            assert_eq!(
                serde_json::to_value(&updated.instruments[0]).unwrap(),
                serde_json::to_value(&wildcard).unwrap()
            );
        }
        let mut missing = array.clone();
        missing["instruments"] = serde_json::json!([]);
        assert!(market_liquidator_action(missing, "{}", "FOO-USD").is_err());
        let mut duplicate = array;
        duplicate["instruments"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(wildcard).unwrap());
        assert!(market_liquidator_action(duplicate, "{}", "FOO-USD").is_err());
    }

    #[test]
    fn discovers_tier_references_and_substitutes_new_symbol() {
        let root = std::env::temp_dir().join(format!(
            "bulk-create-market-{}-{}",
            std::process::id(),
            time_epoch_ns()
        ));
        std::fs::create_dir_all(root.join("securities")).unwrap();
        std::fs::write(
            root.join("securities/Currencies.json"),
            r#"[{"name":"FOO","type":"Currency"}]"#,
        )
        .unwrap();
        std::fs::write(
            root.join("securities/Perpetuals.json"),
            r#"[{"name":"FOO-USD","type":"Perpetual","pccy":"FOO","qccy":"USD"}]"#,
        )
        .unwrap();
        let executor = root.join("executor.json5");
        std::fs::write(&executor, "// JSON5\n{funding: {},}").unwrap();
        std::fs::create_dir_all(root.join("risk/ref")).unwrap();
        for tier in ["low", "medium", "high"] {
            std::fs::write(root.join(format!("risk/ref/{tier}.csv")), "fixture").unwrap();
            for kind in ["volatility", "fairprice", "regime"] {
                let directory = root.join(format!("models/{kind}/ref"));
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    directory.join(format!("{tier}.json5")),
                    r#"{"instrument":"SYMBOL-USD"}"#,
                )
                .unwrap();
            }
            let plan = read_market_plan(&root, &executor, "FOO", Some(tier), false).unwrap();
            assert_eq!(plan.securities.len(), 2);
            assert_eq!(plan.securities[0]["type"], "Currency");
            assert_eq!(plan.securities[1]["type"], "Perpetual");
            for model in [plan.volatility, plan.fairprice, plan.regime] {
                assert!(model.contains("FOO-USD") && !model.contains("SYMBOL"));
            }
            assert!(plan.risk_csv.ends_with(format!("{tier}.csv")));
        }
        assert_eq!(
            read_market_plan(&root, &executor, "FOO", Some("low"), true)
                .unwrap()
                .securities
                .len(),
            1
        );
        assert!(read_market_plan(&root, &executor, "BAR", Some("low"), false).is_err());
        assert!(read_market_plan(&root, &executor, "../FOO", Some("low"), false).is_err());
        std::fs::remove_file(root.join("models/regime/ref/low.json5")).unwrap();
        assert!(read_market_plan(&root, &executor, "FOO", Some("low"), false).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn security_selection_rejects_missing_duplicate_and_wrong_type() {
        let currency = serde_json::json!({"name":"FOO", "type":"Currency"});
        assert!(select_security_definition(vec![currency.clone()], "FOO", "Currency").is_ok());
        assert!(select_security_definition(vec![currency.clone()], "BAR", "Currency").is_err());
        assert!(select_security_definition(vec![currency.clone()], "FOO", "Perpetual").is_err());
        assert!(
            select_security_definition(vec![currency.clone(), currency], "FOO", "Currency")
                .is_err()
        );
    }

    #[test]
    fn funding_uses_pair_override_or_wildcard_without_merging() {
        let default = serde_json::json!({"rate":0.125,"deviation_cap":0.625,"funding_cap":30,
            "premium_horizon":8,"notional":25000,"sample_period":5,"mean_window":720});
        let mut pair = default.clone();
        pair["notional"] = serde_json::json!(1000);
        let config = serde_json::json!({"funding":{"*":default,"FOO-USD":pair}});
        let raw = config.to_string();
        let direct = funding_config_for(&raw, "FOO-USD").unwrap();
        assert_eq!(direct.notional, 1000.0);
        assert_eq!(direct.symbol, "FOO-USD");
        let fallback = funding_config_for(&raw, "BAR-USD").unwrap();
        assert_eq!(fallback.notional, 25000.0);
        assert_eq!(fallback.symbol, "BAR-USD");
        let mut invalid = config;
        invalid["funding"]["FOO-USD"]
            .as_object_mut()
            .unwrap()
            .remove("premium_horizon");
        assert!(funding_config_for(&invalid.to_string(), "FOO-USD").is_err());
        assert!(funding_config_for("{funding: {},}", "FOO-USD").is_err());
        assert!(funding_config_for("{}", "FOO-USD").is_err());
        assert!(funding_config_for("// JSON5 comment\n{funding:{'*':null,},}", "FOO-USD").is_err());
    }

    #[test]
    fn wire_proxy_matches_native_sdk_market_id_encoding() {
        let sdk_id = MktId {
            uuid: 0x0102_0304_0506_0708,
        };

        assert_eq!(
            bincode::serialize(&sdk_id).expect("serialize SDK market ID"),
            bincode::serialize(&wire_mktid(sdk_id)).expect("serialize wire market ID")
        );
    }
}
