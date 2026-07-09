use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bulk_client::msgs::{AddMarket, Matrix, OpaqueAction};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;
use bulk_sdk_core::data::models::{BarModelConfig, TickModelConfig};
use bulk_sdk_core::markets::MktId;
use bulk_sdk_core::models::margin::risk_config::RiskConfig;
use bulk_sdk_core::models::margin::risk_matrix::RiskMatrix;
use bulk_sdk_core::models::signals::{WrappedBarFunction1D, WrappedTickFunctionMD};
use bulk_sdk_core::securities::Security;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::commands::{
    AddMarketArgs, ConfigModelArgs, ConfigRiskMatrixArgs, ConfigSecurityArgs, CorrsArgs,
};
use crate::common::submit::{submit_actions, SubmitOptions};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}

fn read_json5<T: DeserializeOwned>(path: &str, label: &str) -> eyre::Result<T> {
    json5::from_str(
        &std::fs::read_to_string(path)
            .map_err(|error| eyre::eyre!("failed to read '{path}': {error}"))?,
    )
    .map_err(|error| eyre::eyre!("invalid {label} '{path}': {error}"))
}

fn mktid(name: &str) -> eyre::Result<MktId> {
    MktId::new(name).ok_or_else(|| {
        eyre::eyre!("unknown market id '{name}' (pass --securities before model/risk commands)")
    })
}

fn opaque<T: Serialize>(value: &T, label: &str) -> eyre::Result<OpaqueAction> {
    Ok(OpaqueAction {
        payload: bincode::serialize(value)
            .map_err(|error| eyre::eyre!("bincode {label}: {error}"))?,
        meta: Default::default(),
    })
}

fn default_settle_ccy() -> String {
    "USD".to_string()
}

fn default_corr_discount() -> f64 {
    1.0
}

#[derive(serde::Deserialize)]
struct RiskMatrixBudget {
    #[serde(default = "default_settle_ccy")]
    settle_ccy: String,
    max_loss: f64,
    eloss_floor: f64,
    max_pliq: f64,
    #[serde(default)]
    margin_buffer: f64,
    #[serde(default = "default_corr_discount")]
    corr_discount: f64,
}

impl RiskMatrixBudget {
    fn into_risk_config(self) -> eyre::Result<RiskConfig> {
        Ok(RiskConfig {
            settle_ccy: mktid(&self.settle_ccy)?,
            max_loss: self.max_loss,
            eloss_floor: self.eloss_floor,
            max_pliq: self.max_pliq,
            margin_buffer: self.margin_buffer,
            corr_discount: self.corr_discount,
        })
    }
}

pub fn register_securities(path: &str) -> eyre::Result<()> {
    for security in read_json5::<Vec<Security>>(path, "securities config")? {
        Security::register(Arc::new(security), true)?;
    }
    Ok(())
}

pub async fn handle_config_security(
    api: &mut BulkHttpClient,
    args: ConfigSecurityArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let securities = read_json5::<Vec<Security>>(&args.json, "securities config")?;
    let mut actions = Vec::with_capacity(securities.len());

    for security in securities {
        actions.push(Action::ConfigSecurity(opaque(&security, "security")?));
    }

    eprintln!("Placing {} security config action(s)", actions.len());
    submit_actions(api, submit, actions).await
}

pub async fn handle_config_fairprice(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    eprintln!("Placing fair-price config for {}", args.coin);
    submit_actions(
        api,
        submit,
        vec![Action::ConfigFairPrice(opaque(
            &TickModelConfig {
                stamp: now_ns(),
                instrument: mktid(&args.coin)?,
                signal: read_json5::<WrappedTickFunctionMD>(&args.json, "fair-price model")?,
            },
            "fair-price config",
        )?)],
    )
    .await
}

pub async fn handle_config_regime(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    eprintln!("Placing regime config for {}", args.coin);
    submit_actions(
        api,
        submit,
        vec![Action::ConfigRegime(opaque(
            &BarModelConfig {
                stamp: now_ns(),
                instrument: mktid(&args.coin)?,
                signal: read_json5::<WrappedBarFunction1D>(&args.json, "regime model")?,
            },
            "regime config",
        )?)],
    )
    .await
}

pub async fn handle_config_volatility(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    eprintln!("Placing volatility config for {}", args.coin);
    submit_actions(
        api,
        submit,
        vec![Action::ConfigVolatility(opaque(
            &BarModelConfig {
                stamp: now_ns(),
                instrument: mktid(&args.coin)?,
                signal: read_json5::<WrappedBarFunction1D>(&args.json, "volatility model")?,
            },
            "volatility config",
        )?)],
    )
    .await
}

pub async fn handle_config_risk_matrix(
    api: &mut BulkHttpClient,
    args: ConfigRiskMatrixArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let risk_config =
        read_json5::<RiskMatrixBudget>(&args.risk_config, "risk budget")?.into_risk_config()?;

    eprintln!("Placing risk matrix for {}", args.coin);
    submit_actions(
        api,
        submit,
        vec![Action::ConfigRisk(opaque(
            &RiskMatrix::from_csv(now_ns(), mktid(&args.coin)?, &risk_config, &args.csv)
                .map_err(|error| eyre::eyre!("risk matrix from '{}': {error}", args.csv))?,
            "risk matrix",
        )?)],
    )
    .await
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
