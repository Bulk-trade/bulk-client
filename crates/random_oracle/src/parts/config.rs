use std::path::Path;
use eyre::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PriceConfig {
    /// Coin to produce prices for
    pub coin: String,

    /// Base price / long-run mean for OU process
    #[serde(default = "default_price")]
    pub price: f64,

    /// Mean-reversion speed (e.g. 0.0001)
    #[serde(default = "default_kappa")]
    pub kappa: f64,

    /// Volatility (e.g. 0.01)
    #[serde(default = "default_sigma")]
    pub sigma: f64,
}

/// Synthetic oracle config
#[derive(Debug, Clone, Default,Deserialize)]
pub struct OracleConfig {
    /// Api url
    #[serde(default = "default_url")]
    pub url: String,
    /// oracle set
    pub coins: Vec<PriceConfig>,
    /// update frequency (in seconds)
    pub frequency: f64,
    /// Post timeout (seconds)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

impl OracleConfig {
    /// Load config from a JSON5 file.
    ///
    /// # Arguments
    /// - `path`: path to config file
    pub fn load(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .wrap_err_with(|| format!("reading config file {:?}", path.as_ref()))?;
        let config: OracleConfig = json5::from_str(&text)
            .wrap_err("parsing JSON5 config")?;
        Ok(config)
    }
}


// ─────────────────────────────────────────────────────────────────────
// Defaults
// ─────────────────────────────────────────────────────────────────────

fn default_url() -> String { "ws://localhost:12001".into() }
fn default_price() -> f64 { 100_000.0 }
fn default_kappa() -> f64 { 0.0001 }
fn default_sigma() -> f64 { 0.01 }
fn default_timeout() -> u64 { 10 }