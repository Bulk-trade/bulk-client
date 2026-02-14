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
//! use bulk_sdk::*;
//!
//! #[tokio::main]
//! async fn main() -> eyre::Result<()> {
//!     // Read-only client (market data + account queries)
//!     let client = BulkHttpClient::new(None)?;
//!
//!     let ticker = client.get_ticker("BTC-USD").await?;
//!     println!("BTC mark: {}", ticker.mark_price);
//!
//!     // Authenticated client (trading)
//!     let client = BulkHttpClient::new(Some("your_base58_private_key"))?;
//!     let resp = client.place_limit_order(
//!         "BTC-USD", Side::Buy, 95_000.0, 0.001,
//!         TimeInForce::GTC, false,
//!     ).await?;
//!     Ok(())
//! }
//! ```

use std::time::Duration;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use solana_pubkey::Pubkey;
use crate::api::parts::{make_nonce, HttpConfig};
use crate::common::side::Side;
use crate::common::tif::TimeInForce;
use crate::common::TransactionSigner;
use crate::msgs::*;
use crate::tx::order_tx::{OrderAction, OrderTransaction};

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
}

#[allow(unused)]
impl BulkHttpClient {
    /// Create bulk HTTP client
    ///
    /// # Arguments
    /// - `config`: http client config
    pub fn new (config: &HttpConfig) -> eyre::Result<Self> {
        let client = Client::builder()
            .timeout(config.default_timeout)
            .build()?;

        Ok(Self {
            config: config.clone(),
            client
        })
    }

    /// Create bulk HTTP client with url, private key
    ///
    /// # Arguments
    /// - `base_url`: http client url
    /// - `private_key`: optional private key
    pub fn with_url (base_url: &str, private_key: Option<&str>) -> eyre::Result<Self> {
        if let Some(private_key) = private_key {
            let signer = TransactionSigner::from_private_key(private_key)?;
            let config = HttpConfig {
                base_url: base_url.to_string(),
                signer: Some(signer),
                default_timeout: Duration::from_secs(10)
            };
            Self::new(&config)
        } else {
            let config = HttpConfig {
                base_url: base_url.to_string(),
                signer: None,
                default_timeout: Duration::from_secs(10)
            };
            Self::new(&config)
        }
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

    /// Get trade history (up to 5000 recent fills).
    ///
    /// # Arguments
    /// - `user`: user pubkey to query
    pub async fn get_fills(&self, user: &str) -> eyre::Result<Vec<Fill>> {
        let resp = self
            .client
            .post(format!("{}/account", self.config.base_url))
            .json(&json!({ "type": "fills", "user": user }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get closed position history (up to 5000 positions).
    ///
    /// # Arguments
    /// - `user`: user pubkey to query
    pub async fn get_position_history(&self, user: &str) -> eyre::Result<Vec<PositionInfo>> {
        let resp = self
            .client
            .post(format!("{}/account", self.config.base_url))
            .json(&json!({ "type": "positions", "user": user }))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    // =====================================================================
    // Trading (SIGNED)
    // =====================================================================

    /// Place multiple order actions in a single signed transaction.
    ///
    /// Accepts any mix of limit orders, market orders, cancels, and cancel-alls.
    ///
    /// # Example
    /// ```rust,no_run
    /// let resp = client.place_orders(vec![
    ///     LimitOrder::new("BTC-USD", Side::Buy, 95_000.0, 0.01, TimeInForce::GTC).into(),
    ///     CancelAll::new(vec!["ETH-USD".into()]).into(),
    /// ], None).await?;
    /// ```
    pub async fn place_orders(
        &self,
        actions: Vec<OrderAction>,
        nonce: Option<u64>,
    ) -> eyre::Result<Vec<OrderResponse>> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for trading operations"))?;

        let nonce = nonce.unwrap_or_else(make_nonce);
        let pk = signer.public_key();

        // Build + sign the transaction
        let mut bundle = OrderTransaction::new(actions, nonce, pk, pk);
        signer.sign(&mut bundle)?;

        let sig = bundle
            .signature_b58()
            .ok_or_else(|| eyre::eyre!("signing failed"))?;
        let pk_b58 = signer.public_key_b58();

        // Build orders JSON array via write_api
        let mut orders_buf = String::with_capacity(256 * bundle.actions.len());
        orders_buf.push('[');
        for (i, action) in bundle.actions.iter().enumerate() {
            if i > 0 { orders_buf.push(','); }
            action.write_api(&mut orders_buf);
        }
        orders_buf.push(']');

        // Assemble full transaction JSON string
        let body = format!(
            r#"{{"action":{{"type":"order","orders":{},"nonce":{}}},"account":"{}","signer":"{}","signature":"{}"}}"#,
            orders_buf, nonce, pk_b58, pk_b58, sig,
        );

        let resp = self
            .client
            .post(format!("{}/order", self.config.base_url))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        let data: Value = resp.json().await?;
        Ok(OrderResponse::parse_responses(&data))
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
    ) -> eyre::Result<OrderResponse> {
        let mut order = LimitOrder::new(symbol, side, price, size, tif);
        order.reduce_only = reduce_only;

        let results = self.place_orders(vec![order.into()], None).await?;
        Ok(results[0].clone())
    }

    /// Place a single market order.
    pub async fn place_market_order(
        &self,
        symbol: &str,
        side: Side,
        size: f64,
        reduce_only: bool,
    ) -> eyre::Result<OrderResponse> {
        let mut order = MarketOrder::new(symbol, side, size);
        order.reduce_only = reduce_only;

        let results = self.place_orders(vec![order.into()], None).await?;
        Ok(results[0].clone())
    }

    /// Cancel a single order by ID.
    pub async fn cancel_order(
        &self,
        symbol: &str,
        order_id: &str,
    ) -> eyre::Result<OrderResponse> {
        let cancel = CancelOrder::new(symbol, order_id);

        let results = self.place_orders(vec![cancel.into()], None).await?;
        Ok(results[0].clone())
    }

    /// Cancel all orders, optionally filtered by symbols.
    pub async fn cancel_all(&self, symbols: Vec<String>) -> eyre::Result<OrderResponse> {
        let cancel = CancelAll::new(symbols);

        let results = self.place_orders(vec![cancel.into()], None).await?;
        Ok(results[0].clone())
    }

    // =====================================================================
    // Meta (SIGNED)
    // =====================================================================

    /// Update maximum leverage settings for markets.
    ///
    /// # Arguments
    /// - `settings`: List of (symbol, max_leverage) pairs
    pub async fn update_leverage(
        &self,
        settings: &[(&str, f64)],
    ) -> eyre::Result<Value> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for settings operations"))?;

        let leverage_list: Vec<Value> = settings
            .iter()
            .map(|(sym, lev)| json!([sym, lev]))
            .collect();

        let transaction = self.sign_generic_transaction(json!({
            "action": {
                "type": "updateUserSettings",
                "settings": { "m": leverage_list },
                "nonce": 1,
            },
        }))?;

        let resp = self
            .client
            .post(format!("{}/user-settings", self.config.base_url))
            .json(&transaction)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
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
    ) -> eyre::Result<Value> {
        let transaction = self.sign_generic_transaction(json!({
            "action": {
                "type": "agentWalletCreation",
                "agent": {
                    "a": agent_pubkey,
                    "d": delete,
                },
            },
        }))?;

        let resp = self
            .client
            .post(format!("{}/agent-wallet", self.config.base_url))
            .json(&transaction)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
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
        nonce: Option<u64>,
    ) -> eyre::Result<Value> {
        let nonce = nonce.unwrap_or_else(make_nonce);

        let transaction = self.sign_generic_transaction(json!({
            "action": {
                "type": "testnetAdmin",
                "actions": [{
                    "whitelistFaucet": {
                        "account": target_account,
                        "whitelist": whitelist,
                    }
                }],
                "nonce": nonce,
            },
        }))?;

        let resp = self
            .client
            .post(format!("{}/private/testnet-admin", self.config.base_url))
            .json(&transaction)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
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
    ) -> eyre::Result<Value> {
        let signer = self
            .config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("Private key required for faucet operations"))?;

        let nonce = nonce.unwrap_or_else(make_nonce);
        let target_user = user.unwrap_or(signer.public_key());

        let mut faucet = json!({ "u": target_user });
        if let Some(amt) = amount {
            faucet["amount"] = json!(amt);
        }

        // Account field uses the target user (matches Python)
        let mut tx = json!({
            "action": {
                "type": "faucet",
                "faucet": faucet,
                "nonce": nonce,
            },
            "account": target_user,
            "signer": signer.public_key_b58(),
        });

        // Sign and attach signature
        let sig = self.sign_action_payload(&tx["action"])?;
        tx["signature"] = json!(sig);

        let resp = self
            .client
            .post(format!("{}/private/faucet", self.config.base_url))
            .json(&tx)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }


    // =====================================================================
    // Internal helpers
    // =====================================================================

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
        // the exchange expects a signature over the canonical JSON.
        let message = serde_json::to_string(action)?;
        let sig = signer.sign_bytes(message.as_bytes());
        Ok(bs58::encode(sig).into_string())
    }
}