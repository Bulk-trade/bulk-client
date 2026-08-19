//! Bulk Labs HTTP REST API Client
//!
//! Provides complete HTTP REST API access to the Bulk Labs exchange:
//! - Market data endpoints (unsigned)
//! - Account query endpoints (unsigned)
//! - Trading endpoints (signed)
//! - Private endpoints (signed — faucet, etc.)
//!
//! # Example
//!
//! ```rust,no_run
//! use bulk_client::*;
//! use bulk_client::common::side::Side;
//! use bulk_client::common::tif::TimeInForce;
//! use bulk_client::transaction::SignatureDomain;
//!
//! #[tokio::main]
//! async fn main() -> eyre::Result<()> {
//!     // Read-only client (market data + account queries)
//!     let client = BulkHttpClient::with_url(
//!         "https://exchange-api.bulk.trade/api/v1", None, None,
//!     )?;
//!
//!     let ticker = client.get_ticker("BTC-USD").await?;
//!     println!("BTC mark: {}", ticker.mark_price);
//!
//!     // Authenticated client (trading)
//!     let client = BulkHttpClient::with_url(
//!         "https://exchange-api.bulk.trade/api/v1",
//!         Some("your_base58_private_key"),
//!         Some(SignatureDomain::Mainnet),
//!     )?;
//!     let resp = client.place_limit_order(
//!         "BTC-USD", Side::Buy, 95_000.0, 0.001,
//!         TimeInForce::GTC, false, None, None,
//!     ).await?;
//!     Ok(())
//! }
//! ```

use crate::api::parts::{make_nonce, HttpConfig};
use crate::common::side::Side;
use crate::common::tif::TimeInForce;
use crate::msgs::*;
use crate::transaction::{Action, ActionMeta, SignatureDomain, Transaction, TransactionSigner};
use reqwest::{Client, Response as HttpResponse, Url};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/// HTTP REST API client for Bulk Labs exchange.
///
/// Supports both public (unsigned) and private (signed) endpoints.
/// Construct with `None` for read-only access or provide a private key
/// for trading operations.
#[derive(Clone)]
#[allow(unused)]
pub struct BulkHttpClient {
    config: HttpConfig,
    client: Client,
    is_localhost: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_signer_requires_explicit_signature_domain() {
        let error = BulkHttpClient::new(&HttpConfig {
            base_url: "http://localhost".to_string(),
            signer: Some(
                TransactionSigner::from_private_key("1111111111111111111111111111111111111111111")
                    .expect("test signer"),
            ),
            signature_domain: None,
            default_timeout: Duration::from_secs(1),
        })
        .err()
        .expect("missing domain must fail");

        assert!(error.to_string().contains("signature domain is required"));
    }
}

#[allow(unused)]
impl BulkHttpClient {
    /// Create bulk HTTP client
    ///
    /// # Arguments
    /// - `config`: http client config
    pub fn new(config: &HttpConfig) -> eyre::Result<Self> {
        if config.signer.is_some() && config.signature_domain.is_none() {
            return Err(eyre::eyre!(
                "signature domain is required when an HTTP signer is configured"
            ));
        }
        let client = Client::builder().timeout(config.default_timeout).build()?;

        let is_localhost = Self::is_localhost(&config.base_url);

        Ok(Self {
            config: config.clone(),
            client,
            is_localhost,
        })
    }

    /// Create bulk HTTP client with url, private key
    ///
    /// # Arguments
    /// - `base_url`: http client url
    /// - `private_key`: optional private key
    pub fn with_url(
        base_url: &str,
        private_key: Option<&str>,
        signature_domain: Option<SignatureDomain>,
    ) -> eyre::Result<Self> {
        if let Some(private_key) = private_key {
            let signer = TransactionSigner::from_private_key(private_key)?;
            let config = HttpConfig {
                base_url: base_url.to_string(),
                signer: Some(signer),
                signature_domain,
                default_timeout: Duration::from_secs(10),
            };
            Self::new(&config)
        } else {
            let config = HttpConfig {
                base_url: base_url.to_string(),
                signer: None,
                signature_domain: None,
                default_timeout: Duration::from_secs(10),
            };
            Self::new(&config)
        }
    }

    /// Create bulk HTTP client with a pre-built signer (software key or Ledger).
    ///
    /// # Example
    /// ```text
    /// let signer = TransactionSigner::from_ledger("usb://ledger", None)?;
    /// let client = BulkHttpClient::with_signer(
    ///     "https://exchange-api.bulk.trade/api/v1",
    ///     signer,
    ///     SignatureDomain::Mainnet,
    /// )?;
    /// let resp = client.request_faucet(None, None, None).await?;
    /// ```
    pub fn with_signer(
        base_url: &str,
        signer: TransactionSigner,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Self> {
        let config = HttpConfig {
            base_url: base_url.to_string(),
            signer: Some(signer),
            signature_domain: Some(signature_domain),
            default_timeout: Duration::from_secs(10),
        };
        Self::new(&config)
    }

    /// Channel configuration
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }

    /// Pubkeyt associated with this channel
    pub fn public_key(&self) -> Option<Pubkey> {
        self.config.signer.as_ref().map(|x| x.public_key())
    }

    // =====================================================================
    // MARKET DATA ENDPOINTS (PUBLIC, UNSIGNED)
    // =====================================================================

    /// Get exchange information including all available markets.
    pub async fn get_exchange_info(&self) -> eyre::Result<Vec<MarketInfo>> {
        let resp = self
            .client
            .get(format!("{}/exchangeInfo", self.config.base_url))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get market ticker/statistics for a symbol.
    pub async fn get_ticker(&self, symbol: &str) -> eyre::Result<Ticker> {
        let resp = self
            .client
            .get(format!("{}/ticker/{}", self.config.base_url, symbol))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get historical candlestick/OHLCV data.
    ///
    /// # Arguments
    /// - `symbol`: Market symbol (e.g. "BTC-USD")
    /// - `interval`: Candle interval ("1m", "5m", "15m", "30m", "1h", "4h", "1d", "1w")
    /// - `start_time`: Optional start timestamp in milliseconds
    /// - `end_time`: Optional end timestamp in milliseconds
    /// - `limit`: Maximum candles to return (default 500, max 1000)
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: Option<u64>,
        end_time: Option<u64>,
        limit: Option<u32>,
    ) -> eyre::Result<Vec<Candle>> {
        let mut params = vec![
            ("symbol".to_string(), symbol.to_string()),
            ("interval".to_string(), interval.to_string()),
            ("limit".to_string(), limit.unwrap_or(500).to_string()),
        ];
        if let Some(st) = start_time {
            params.push(("startTime".to_string(), st.to_string()));
        }
        if let Some(et) = end_time {
            params.push(("endTime".to_string(), et.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/klines", self.config.base_url))
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get L2 order book snapshot.
    ///
    /// # Arguments
    /// - `symbol`: Market symbol
    /// - `nlevels`: Number of price levels per side (default 20, max 1000)
    /// - `aggregation`: Optional price aggregation/grouping
    pub async fn get_orderbook(
        &self,
        symbol: &str,
        nlevels: Option<u32>,
        aggregation: Option<f64>,
    ) -> eyre::Result<L2Snapshot> {
        let mut params = vec![
            ("type".to_string(), "l2Book".to_string()),
            ("coin".to_string(), symbol.to_string()),
            ("nlevels".to_string(), nlevels.unwrap_or(20).to_string()),
        ];
        if let Some(agg) = aggregation {
            params.push(("aggregation".to_string(), agg.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/l2book", self.config.base_url))
            .query(&params)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    // =====================================================================
    // ACCOUNT ENDPOINTS (PUBLIC, UNSIGNED)
    // =====================================================================

    /// Get complete account state including positions, orders, and margin.
    ///
    /// # Arguments
    /// - `user`: user pubkey to query
    pub async fn get_account(&self, user: Pubkey) -> eyre::Result<AccountData> {
        #[derive(Debug, Clone, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct FullAccountResponse {
            pub full_account: AccountData,
        }

        let user: String = user.to_string();

        let resp = self
            .client
            .post(format!("{}/account", self.config.base_url))
            .json(&json!({ "type": "fullAccount", "user": user }))
            .send()
            .await?
            .error_for_status()?;

        let arr: Vec<FullAccountResponse> = resp.json().await?;
        arr.into_iter()
            .next()
            .map(|r| r.full_account)
            .ok_or_else(|| eyre::eyre!("empty fullAccount response"))
    }

    /// Get resting orders for an account.
    ///
    /// # Arguments
    /// - `user`: user pubkey to query
    pub async fn get_open_orders(&self, user: &str) -> eyre::Result<Vec<OrderState>> {
        let resp = self
            .client
            .post(format!("{}/account", self.config.base_url))
            .json(&json!({ "type": "openOrders", "user": user }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get one bounded page of account fills.
    pub async fn get_fills_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<HistoryFill>, HistoryHttpError> {
        self.get_history_page(user, "fills", query).await
    }

    /// Get one bounded page of closed positions.
    pub async fn get_positions_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<ClosedPosition>, HistoryHttpError> {
        self.get_history_page(user, "positions", query).await
    }

    /// Get one bounded page of funding payments.
    pub async fn get_funding_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<FundingPayment>, HistoryHttpError> {
        self.get_history_page(user, "fundingHistory", query).await
    }

    /// Get one bounded page of terminal orders.
    pub async fn get_orders_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<TerminalOrder>, HistoryHttpError> {
        self.get_history_page(user, "orderHistory", query).await
    }

    /// Get one bounded page of account activity.
    pub async fn get_activity_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<AccountActivity>, HistoryHttpError> {
        self.get_history_page(user, "activityHistory", query).await
    }

    /// Get one bounded page of account risk events.
    pub async fn get_risk_page(
        &self,
        user: &str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<RiskEvent>, HistoryHttpError> {
        self.get_history_page(user, "riskHistory", query).await
    }

    // =====================================================================
    // Trading (SIGNED)
    // =====================================================================

    /// Place multiple order actions in a single signed transaction.
    ///
    /// Accepts any mix of limit orders, market orders, cancels, and cancel-alls.
    ///
    /// # Example
    /// ```text
    /// let resp = client.place_tx(vec![
    ///     Action::LimitOrder(LimitOrder { .. }),
    ///     Action::CancelAll(CancelAll { .. }),
    /// ], None, None).await?;
    /// ```
    pub async fn place_tx(
        &self,
        actions: Vec<Action>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Vec<Response>> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        // Build + sign the transaction
        let mut tx = Transaction {
            actions,
            nonce,
            account: account,
            signer: signer.public_key(),
            signature: Default::default(),
        };
        tx.sign(
            signer,
            self.config
                .signature_domain
                .ok_or_else(|| eyre::eyre!("signature domain is required for signed requests"))?,
        )?;

        // Build JSON body via tx serialization
        let body = serde_json::to_string(&tx)?;

        let mut request = self
            .client
            .post(format!("{}/order", self.config.base_url))
            .header("content-type", "application/json");
        if let Some(mode) = signer.tx_signature_mode_hint_header_value() {
            request = request.header("X-Bulk-Sig-Mode", mode);
        }
        let resp = request.body(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(eyre::eyre!(
                "HTTP {} from /order: {}",
                status,
                text.chars().take(600).collect::<String>()
            ));
        }

        let data: Value = resp.json().await?;
        Ok(Response::parse_responses(&data))
    }

    /// Place a single limit order.
    pub async fn place_limit_order(
        &self,
        symbol: &str,
        side: Side,
        price: f64,
        size: f64,
        tif: TimeInForce,
        reduce_only: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);

        let order = LimitOrder {
            symbol: Arc::from(symbol),
            is_buy: side == Side::Buy,
            price,
            size,
            tif,
            reduce_only,
            iso: false,
            builder_code: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![order.into()], None, None).await?;
        Ok(results[0].clone())
    }

    /// Place a single market order.
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: Side,
        size: f64,
        reduce_only: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);

        let order = MarketOrder {
            symbol: Arc::from(symbol),
            is_buy: side == Side::Buy,
            size,
            reduce_only,
            iso: false,
            builder_code: None,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![order.into()], None, None).await?;
        Ok(results[0].clone())
    }

    /// Cancel a single order by ID.
    pub async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let cancel = CancelOrder {
            symbol: symbol.to_string(),
            oid: Hash::from_str(&order_id)?,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![cancel.into()], None, None).await?;
        Ok(results[0].clone())
    }

    /// Cancel all orders, optionally filtered by symbols.
    pub async fn cancel_all(
        &self,
        symbols: Vec<String>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let cancel = CancelAll {
            symbols,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![cancel.into()], None, None).await?;
        Ok(results[0].clone())
    }

    // =====================================================================
    // Meta (SIGNED)
    // =====================================================================

    /// Update maximum leverage settings for markets.
    ///
    /// # Arguments
    /// - `settings`: Map of (symbol, max_leverage) pairs
    pub async fn update_leverage(
        &self,
        settings: HashMap<String, f64>,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let settings = UpdateUserSettings {
            max_leverage: settings,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![settings.into()], None, None).await?;
        Ok(results[0].clone())
    }

    /// Create or delete an agent wallet authorization.
    ///
    /// # Arguments
    /// - `agent_pubkey`: Agent's public key (base58)
    /// - `delete`: `true` to remove the agent, `false` to add
    pub async fn manage_agent_wallet(
        &self,
        agent_pubkey: Pubkey,
        delete: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let settings = AgentWalletCreation {
            agent: agent_pubkey,
            delete,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self
            .place_tx(vec![Action::AgentWalletCreation(settings)], None, None)
            .await?;
        Ok(results[0].clone())
    }

    /// Approve a builder-code recipient for routed orders.
    ///
    /// Builder codes are encoded as builder-code fees on the wire.
    pub async fn approve_builder_code(
        &self,
        to: Pubkey,
        fee: u8,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let action = ApproveCommissionFee {
            to,
            max_fee: fee,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![action.into()], None, None).await?;
        Ok(results[0].clone())
    }

    /// Revoke a builder-code recipient approval.
    pub async fn revoke_builder_code(
        &self,
        to: Pubkey,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let action = RevokeCommissionFee {
            to,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![action.into()], None, None).await?;
        Ok(results[0].clone())
    }

    // =====================================================================
    // Testnet-only (SIGNED)
    // =====================================================================

    /// Whitelist or unwhitelist an account for testnet faucet access.
    ///
    /// **Testnet admin only.**
    ///
    /// # Arguments
    /// - `target_account`: account to be whitelisted
    /// - `whitelist`: if true is added whitelisted, if false removed
    /// - `nonce`: tx nonce
    pub async fn whitelist_faucet(
        &self,
        target_account: Pubkey,
        whitelist: bool,
        account: Option<Pubkey>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let account = if let Some(account) = account {
            account
        } else {
            signer.public_key()
        };

        let nonce = nonce.unwrap_or_else(make_nonce);
        let settings = WhitelistFaucet {
            target: target_account,
            whitelist,
            meta: ActionMeta {
                account,
                nonce,
                seqno: 0,
                hash: None,
            },
        };
        let results = self
            .place_tx(vec![Action::WhitelistFaucet(settings)], None, None)
            .await?;
        Ok(results[0].clone())
    }

    /// Request testnet faucet funds.
    ///
    /// **Testnet only.**
    ///
    /// # Arguments
    /// - `user`: Optional target user public key (defaults to signer's key)
    /// - `amount`: Optional specific amount (only for whitelisted accounts)
    /// - `nonce`: Optional nonce
    pub async fn request_faucet(
        &self,
        user: Option<Pubkey>,
        amount: Option<f64>,
        nonce: Option<u64>,
    ) -> eyre::Result<Response> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let user = if let Some(user) = user {
            user
        } else {
            signer.public_key()
        };
        let nonce = nonce.unwrap_or_else(make_nonce);

        let req = Faucet {
            user,
            amount,
            meta: ActionMeta {
                account: user,
                nonce,
                seqno: 0,
                hash: None,
            },
        };

        let results = self.place_tx(vec![Action::Faucet(req)], None, None).await?;
        Ok(results[0].clone())
    }

    // ───── Internal Request Helpers ─────────────────────────────────────────────────────────

    async fn get_history_page<T: DeserializeOwned>(
        &self,
        user: &str,
        request_type: &'static str,
        query: &HistoryQuery,
    ) -> Result<HistoryPage<T>, HistoryHttpError> {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AccountHistoryRequest<'a> {
            #[serde(rename = "type")]
            request_type: &'static str,
            user: &'a str,
            #[serde(flatten)]
            query: &'a HistoryQuery,
        }

        let response = self
            .client
            .post(format!("{}/account", self.config.base_url))
            .json(&AccountHistoryRequest {
                request_type,
                user,
                query,
            })
            .send()
            .await?;
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(history_api_error(response).await)
        }
    }

    /// determine if is localhost URL
    fn is_localhost(url_str: &str) -> bool {
        let Ok(url) = Url::parse(url_str) else {
            return false;
        };
        match url.host_str() {
            Some("localhost" | "127.0.0.1" | "::1") => true,
            _ => false,
        }
    }

    /// Build a signed transaction envelope from a partial JSON body.
    ///
    /// Adds `account`, `signer`, and `signature` fields.
    fn sign_generic_transaction(&self, mut body: Value) -> eyre::Result<Value> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required"))?;

        let pk_b58 = signer.public_key_b58();
        body["account"] = json!(pk_b58);
        body["signer"] = json!(pk_b58);

        let sig = self.sign_action_payload(&body["action"])?;
        body["signature"] = json!(sig);

        Ok(body)
    }

    /// Sign the `action` portion of a transaction and return the base58 signature.
    ///
    /// This serializes the action JSON to a canonical string and signs it,
    /// matching the Python SDK's `sign_transaction` behavior for generic
    /// (non-order) payloads.
    fn sign_action_payload(&self, action: &Value) -> eyre::Result<String> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required"))?;

        // The Python SDK signs the JSON-serialized action string.
        // For order transactions we use the binary Signable path instead,
        // but for generic endpoints (leverage, faucet, agent wallet, etc.)
        // the exchange expects canonical JSON bound to its configured network.
        let message = serde_json::to_string(action)?;
        let sig = signer.sign_bytes(
            message.as_bytes(),
            self.config
                .signature_domain
                .ok_or_else(|| eyre::eyre!("signature domain is required for signed requests"))?,
        )?;
        Ok(bs58::encode(sig).into_string())
    }
}

const HISTORY_ERROR_BODY_MAX_BYTES: usize = 4 * 1024;

async fn history_api_error(mut response: HttpResponse) -> HistoryHttpError {
    let status = response.status();
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk))
                if bytes.len().saturating_add(chunk.len()) <= HISTORY_ERROR_BODY_MAX_BYTES =>
            {
                bytes.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Ok(Some(_)) | Err(_) => return fallback_history_api_error(status),
        }
    }
    if let Ok(body) = serde_json::from_slice::<HistoryErrorEnvelope>(&bytes) {
        if !body.error.code.is_empty() && !body.error.message.is_empty() {
            return HistoryHttpError::Api { status, body };
        }
    }
    fallback_history_api_error(status)
}

fn fallback_history_api_error(status: reqwest::StatusCode) -> HistoryHttpError {
    HistoryHttpError::Api {
        status,
        body: HistoryErrorEnvelope {
            error: HistoryErrorBody {
                code: "HISTORY_HTTP_ERROR".to_string(),
                message: format!(
                    "History request failed (HTTP {}): invalid error response",
                    status.as_u16()
                ),
            },
        },
    }
}
