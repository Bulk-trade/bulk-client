use std::time::Duration;
use crate::common::TransactionSigner;

/// Bulk API configuration
#[derive(Debug, Clone)]
pub struct WSConfig {
    pub url: String,
    pub symbols: Vec<String>,
    pub signer: Option<TransactionSigner>,
    pub default_timeout: Duration,
}

impl Default for WSConfig {
    fn default() -> Self {
        Self {
            url: "wss://exchange-wss.bulk.trade".into(),
            symbols: vec!["BTC-USD".into(), "ETH-USD".into(), "SOL-USD".into()],
            signer: None,
            default_timeout: Duration::from_secs(10),
        }
    }
}