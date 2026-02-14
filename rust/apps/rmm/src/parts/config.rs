use std::path::Path;
use eyre::Context;
use serde::Deserialize;

/// Random Market Maker configuration (mirrors Python RMMConfig).
#[derive(Debug, Clone, Deserialize)]
pub struct RmmConfig {
    /// Base coin (e.g. "BTC")
    pub coin: String,

    /// WebSocket URL
    #[serde(default = "default_url")]
    pub url: String,

    /// Base price / long-run mean for OU process
    #[serde(default = "default_price")]
    pub price: f64,

    /// Mean-reversion speed (e.g. 0.0001)
    #[serde(default = "default_kappa")]
    pub kappa: f64,

    /// Volatility (e.g. 0.01)
    #[serde(default = "default_sigma")]
    pub sigma: f64,

    /// Half-spread between best bid/ask (e.g. 0.10)
    #[serde(default = "default_spread")]
    pub spread: f64,

    /// Base order size in coins
    #[serde(default = "default_size")]
    pub size: f64,

    /// Number of price levels per side
    #[serde(default = "default_depth")]
    pub depth: usize,

    /// Orders per price level
    #[serde(default = "default_order_per_level")]
    pub order_per_level: usize,

    /// Max actions per bundle chunk
    #[serde(default = "default_chunksize")]
    pub chunksize: usize,

    /// Tick frequency in seconds
    #[serde(default = "default_frequency")]
    pub frequency: f64,

    /// Priming ticks (oracle-only, no orders)
    #[serde(default = "default_priming")]
    pub priming: u64,
}

impl RmmConfig {
    /// The exchange trading symbol (e.g. "BTC-USD").
    pub fn bulk_symbol(&self) -> String {
        format!("{}-USD", self.coin)
    }

    /// Load config from a JSON5 file.
    ///
    /// # Arguments
    /// - `path`: path to config file
    pub fn load(path: impl AsRef<Path>) -> eyre::Result<Self> {
        let text = std::fs::read_to_string(path.as_ref())
            .wrap_err_with(|| format!("reading config file {:?}", path.as_ref()))?;
        let config: RmmConfig = json5::from_str(&text)
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
fn default_spread() -> f64 { 0.10 }
fn default_size() -> f64 { 0.5 }
fn default_depth() -> usize { 500 }
fn default_order_per_level() -> usize { 20 }
fn default_chunksize() -> usize { 500 }
fn default_frequency() -> f64 { 0.010 }
fn default_priming() -> u64 { 100 }