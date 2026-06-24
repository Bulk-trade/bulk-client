use std::time::Duration;
use crate::transaction::TransactionSigner;

/// Bulk Websocket API configuration
#[derive(Debug, Clone)]
pub struct WSConfig {
    pub url: String,
    pub symbols: Vec<String>,
    pub signer: Option<TransactionSigner>,
    pub default_timeout: Duration,
    pub track_account: bool,
    pub track_ticker: bool,
}

/// Bulk HTTP API configuration
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub base_url: String,
    pub signer: Option<TransactionSigner>,
    pub default_timeout: Duration,
}

impl Default for WSConfig {
    fn default() -> Self {
        Self {
            url: "wss://exchange-wss.bulk.trade".into(),
            symbols: vec!["BTC-USD".into(), "ETH-USD".into(), "SOL-USD".into()],
            signer: None,
            default_timeout: Duration::from_secs(120),
            track_ticker: true,
            track_account: true,
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: "https://exchange-api2.bulk.trade/api/v1".into(),
            signer: None,
            default_timeout: Duration::from_secs(120),
        }
    }
}
