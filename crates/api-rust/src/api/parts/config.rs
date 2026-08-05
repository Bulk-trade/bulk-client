use crate::transaction::{SignatureDomain, TransactionSigner};
use std::time::Duration;

/// Bulk Websocket API configuration
#[derive(Debug, Clone)]
pub struct WSConfig {
    pub url: String,
    pub symbols: Vec<String>,
    pub signer: Option<TransactionSigner>,
    pub signature_domain: Option<SignatureDomain>,
    pub default_timeout: Duration,
    /// Maximum size of a complete inbound WebSocket message.
    pub max_message_size: Option<usize>,
    /// Maximum size of a single inbound WebSocket frame.
    pub max_frame_size: Option<usize>,
    pub track_account: bool,
    pub track_ticker: bool,
}

/// Bulk HTTP API configuration
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub base_url: String,
    pub signer: Option<TransactionSigner>,
    pub signature_domain: Option<SignatureDomain>,
    pub default_timeout: Duration,
}

impl Default for WSConfig {
    fn default() -> Self {
        Self {
            url: "wss://exchange-wss.bulk.trade".into(),
            symbols: vec!["BTC-USD".into(), "ETH-USD".into(), "SOL-USD".into()],
            signer: None,
            signature_domain: None,
            default_timeout: Duration::from_secs(10),
            max_message_size: Some(64 << 20),
            max_frame_size: Some(64 << 20),
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
            signature_domain: None,
            default_timeout: Duration::from_secs(10),
        }
    }
}
