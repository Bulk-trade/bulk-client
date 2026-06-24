//! Runtime market-onboarding handlers (Config*/Corrs/AddMarket).
//!
//! Each parses a config file, builds the real bulk-sdk-core struct, bincode-serializes it into the
//! `OpaqueAction` payload exactly as the executor's `update_config` deserializes it, and submits.
//! `meta.account` is set from the signer by `place_tx` (see Action::link), so it isn't set here.
//!
//! NOTE: `MktId::new` resolves names via the global securities registry (`Security::find_by_name`),
//! which is empty in the CLI process. So model/risk commands require the securities to be registered
//! locally first — pass `--securities <file>`, which calls [`register_securities`] before dispatch.
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bulk_client::msgs::{AddMarket, Matrix, OpaqueAction};
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;

use bulk_sdk_core::data::models::{BarModelConfig, TickModelConfig};
use bulk_sdk_core::markets::ids::MktId;
use bulk_sdk_core::models::margin::risk_config::RiskConfig;
use bulk_sdk_core::models::margin::risk_matrix::RiskMatrix;
use bulk_sdk_core::models::signals::{WrappedBarFunction1D, WrappedTickFunctionMD};
use bulk_sdk_core::securities::Security;

use crate::commands::{
    AddMarketArgs, ConfigModelArgs, ConfigRiskMatrixArgs, ConfigSecurityArgs, CorrsArgs,
};
use crate::common::submit::{submit_actions, SubmitOptions};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn read_cfg(path: &str) -> eyre::Result<String> {
    std::fs::read_to_string(path).map_err(|e| eyre::eyre!("failed to read '{}': {e}", path))
}

fn mktid(s: &str) -> eyre::Result<MktId> {
    MktId::new(s).ok_or_else(|| eyre::eyre!("unknown coin '{}' (pass --securities so it resolves)", s))
}

fn opaque(payload: Vec<u8>) -> OpaqueAction {
    OpaqueAction { payload, meta: Default::default() }
}

/// Register securities into the CLI's in-process registry so `MktId::new` resolves coin/perp names.
/// Called from main when `--securities <file>` is given. Idempotent (overwrite).
pub fn register_securities(path: &str) -> eyre::Result<()> {
    let secs: Vec<Security> = json5::from_str(&read_cfg(path)?)
        .map_err(|e| eyre::eyre!("invalid securities file '{}': {e}", path))?;
    for s in secs {
        Security::register(Arc::new(s), true)?;
    }
    Ok(())
}

/// ConfigSecurity — one action per security in file order (currency MUST precede its perp).
pub async fn handle_config_security(
    api: &mut BulkHttpClient,
    args: ConfigSecurityArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let secs: Vec<Security> = json5::from_str(&read_cfg(&args.json)?)
        .map_err(|e| eyre::eyre!("invalid securities config: {e}"))?;
    let mut actions = Vec::with_capacity(secs.len());
    for s in &secs {
        let payload = bincode::serialize(s).map_err(|e| eyre::eyre!("bincode security: {e}"))?;
        actions.push(Action::ConfigSecurity(opaque(payload)));
    }
    eprintln!("Placing {} security config action(s)", actions.len());
    submit_actions(api, submit, actions).await
}

/// ConfigFairPrice — TickModelConfig keyed by COIN, signal = the (1s) fair-price model.
pub async fn handle_config_fairprice(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let signal: WrappedTickFunctionMD = json5::from_str(&read_cfg(&args.json)?)
        .map_err(|e| eyre::eyre!("invalid fair-price model: {e}"))?;
    let cfg = TickModelConfig { stamp: now_ns(), instrument: mktid(&args.coin)?, signal };
    let payload = bincode::serialize(&cfg).map_err(|e| eyre::eyre!("bincode fairprice: {e}"))?;
    eprintln!("Placing fair-price config for {}", args.coin);
    submit_actions(api, submit, vec![Action::ConfigFairPrice(opaque(payload))]).await
}

/// ConfigRegime — BarModelConfig keyed by COIN, signal = the (10s) regime model.
pub async fn handle_config_regime(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let signal: WrappedBarFunction1D = json5::from_str(&read_cfg(&args.json)?)
        .map_err(|e| eyre::eyre!("invalid regime model: {e}"))?;
    let cfg = BarModelConfig { stamp: now_ns(), instrument: mktid(&args.coin)?, signal };
    let payload = bincode::serialize(&cfg).map_err(|e| eyre::eyre!("bincode regime: {e}"))?;
    eprintln!("Placing regime config for {}", args.coin);
    submit_actions(api, submit, vec![Action::ConfigRegime(opaque(payload))]).await
}

/// ConfigVolatility — BarModelConfig keyed by COIN, signal = the (10s) vol metric.
pub async fn handle_config_volatility(
    api: &mut BulkHttpClient,
    args: ConfigModelArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let signal: WrappedBarFunction1D = json5::from_str(&read_cfg(&args.json)?)
        .map_err(|e| eyre::eyre!("invalid volatility model: {e}"))?;
    let cfg = BarModelConfig { stamp: now_ns(), instrument: mktid(&args.coin)?, signal };
    let payload = bincode::serialize(&cfg).map_err(|e| eyre::eyre!("bincode volatility: {e}"))?;
    eprintln!("Placing volatility config for {}", args.coin);
    submit_actions(api, submit, vec![Action::ConfigVolatility(opaque(payload))]).await
}

/// ConfigRisk — build the RiskMatrix from the MC CSV. Only the budget fields of the risk config are
/// needed (max_loss/eloss_floor/max_pliq); the rest of the on-chain RiskConfig (vault/liquidation,
/// which embeds market MktIds) is irrelevant to the matrix and is left at defaults.
#[derive(serde::Deserialize)]
struct Budget {
    max_loss: f64,
    eloss_floor: f64,
    max_pliq: f64,
    #[serde(default)]
    margin_buffer: f64,
}

pub async fn handle_config_risk(
    api: &mut BulkHttpClient,
    args: ConfigRiskMatrixArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let b: Budget = json5::from_str(&read_cfg(&args.risk_config)?)
        .map_err(|e| eyre::eyre!("invalid risk config budget: {e}"))?;
    let rc = RiskConfig {
        max_loss: b.max_loss,
        eloss_floor: b.eloss_floor,
        max_pliq: b.max_pliq,
        margin_buffer: b.margin_buffer,
        ..Default::default()
    };
    let rm = RiskMatrix::from_csv(now_ns(), mktid(&args.coin)?, &rc, &args.csv)
        .map_err(|e| eyre::eyre!("risk matrix from csv: {e}"))?;
    let payload = bincode::serialize(&rm).map_err(|e| eyre::eyre!("bincode risk matrix: {e}"))?;
    eprintln!("Placing risk matrix for {} (budget eloss_floor={})", args.coin, b.eloss_floor);
    submit_actions(api, submit, vec![Action::ConfigRisk(opaque(payload))]).await
}

/// Corrs — full correlation-matrix replace (must include every live coin + the new one).
pub async fn handle_corrs(
    api: &mut BulkHttpClient,
    args: CorrsArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let v: serde_json::Value = json5::from_str(&read_cfg(&args.json)?)
        .map_err(|e| eyre::eyre!("invalid corrs config: {e}"))?;
    let inner = v.get("matrix").cloned().unwrap_or(v);
    let m: Matrix = serde_json::from_value(inner)
        .map_err(|e| eyre::eyre!("invalid correlation matrix: {e}"))?;
    eprintln!("Placing correlation matrix ({} coins)", m.index.len());
    submit_actions(api, submit, vec![Action::Corrs(m)]).await
}

/// AddMarket — create the book (gate requires security + models + corrs already present).
pub async fn handle_add_market(
    api: &mut BulkHttpClient,
    args: AddMarketArgs,
    submit: &SubmitOptions,
) -> eyre::Result<()> {
    let market = AddMarket { symbol: args.symbol.clone().into(), meta: Default::default() };
    eprintln!("Adding market {}", args.symbol);
    submit_actions(api, submit, vec![Action::AddMarket(market)]).await
}
