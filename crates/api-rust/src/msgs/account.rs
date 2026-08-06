use crate::common::order_status::OrderStatus;
use crate::common::order_type::OrderType;
use crate::common::side::Side;
use crate::common::tif::TimeInForce;
use crate::transaction::ActionMeta;
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Faucet Request
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Faucet {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "u")]
    pub user: Pubkey,
    pub amount: Option<f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

// ─────────────────────────────────────────────────────────────────────────────
// Faucet issuer whitelist
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhitelistFaucet {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub target: Pubkey,
    pub whitelist: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

// ─────────────────────────────────────────────────────────────────────────────
// Agent Wallet
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWalletCreation {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "a")]
    pub agent: Pubkey,
    #[serde(rename = "d")]
    pub delete: bool,

    #[serde(skip)]
    pub meta: ActionMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApproveCommissionFee {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "to")]
    pub to: Pubkey,
    #[serde(rename = "fee")]
    pub max_fee: u8,

    #[serde(skip)]
    pub meta: ActionMeta,
}

pub type ApproveBuilderCode = ApproveCommissionFee;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RevokeCommissionFee {
    #[serde(with = "crate::msgs::serde_pubkey", rename = "to")]
    pub to: Pubkey,

    #[serde(skip)]
    pub meta: ActionMeta,
}

pub type RevokeBuilderCode = RevokeCommissionFee;

// ─────────────────────────────────────────────────────────────────────────────
// User Leverage Settings
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateUserSettings {
    #[serde(rename = "m")]
    pub max_leverage: HashMap<String, f64>,

    #[serde(skip)]
    pub meta: ActionMeta,
}

// ─────────────────────────────────────────────────────────────────────────────
// Margin
// ─────────────────────────────────────────────────────────────────────────────

/// Account margin information (from WebSocket `marginUpdate` / `accountSnapshot`).
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(unused)]
pub struct Margin {
    #[serde(rename = "totalMargin", alias = "totalBalance")]
    pub total_margin: f64,
    #[serde(rename = "availableMargin", alias = "availableBalance")]
    pub available_margin: f64,
    #[serde(rename = "executionImpact", default)]
    pub execution_impact: f64,
    #[serde(rename = "transferableBalance", default)]
    pub transferable_balance: f64,
    #[serde(rename = "marginUsed")]
    pub margin_used: f64,
    #[serde(rename = "marginBufferRate", default)]
    pub margin_buffer_rate: f64,
    #[serde(rename = "bufferedMargin", default)]
    pub buffered_margin: f64,
    pub notional: f64,
    #[serde(rename = "realizedPnl")]
    pub realized_pnl: f64,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: f64,
    pub fees: f64,
    pub funding: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Positions
// ─────────────────────────────────────────────────────────────────────────────

/// Position information.
///
/// Deserializes from both WebSocket and HTTP payloads:
/// - WS uses `"symbol"`, HTTP uses `"coin"` → `#[serde(alias)]`
/// - Fields only present on WS default to `0.0` when absent (HTTP).
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct PositionInfo {
    #[serde(alias = "coin")]
    pub symbol: String,
    pub size: f64,
    pub price: f64,
    #[serde(rename = "fairPrice", default)]
    pub fair_price: f64,
    #[serde(default)]
    pub notional: f64,
    #[serde(rename = "realizedPnl", default)]
    pub realized_pnl: f64,
    #[serde(rename = "unrealizedPnl", default)]
    pub unrealized_pnl: f64,
    #[serde(default)]
    pub leverage: f64,
    #[serde(rename = "liquidationPrice", default)]
    pub liquidation_price: f64,
    #[serde(rename = "maintenanceMargin", default)]
    pub maintenance_margin: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Order State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TriggerSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_above: Option<bool>,
    pub px: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lim: Option<f64>,
    pub oco: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub px_hi: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lim_hi: Option<f64>,
    #[serde(rename = "trb", skip_serializing_if = "Option::is_none")]
    pub trail_bps: Option<u32>,
    #[serde(rename = "stb", skip_serializing_if = "Option::is_none")]
    pub step_bps: Option<u32>,
}

/// Resting or historical order state.
///
/// Deserializes from both WebSocket and HTTP payloads:
/// - `vwap`, `reduce_only`, `tif` are HTTP-only (default when absent).
/// - `error` / `reason` is WS-only (default when absent).
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct OrderState {
    #[serde(rename = "ot")]
    pub order_type: OrderType,
    pub status: OrderStatus,
    #[serde(rename = "sym")]
    pub symbol: String,
    #[serde(rename = "oid")]
    pub order_id: String,
    #[serde(rename = "px")]
    pub price: f64,
    #[serde(rename = "origSz")]
    pub original_size: f64,
    #[serde(rename = "sz")]
    pub signed_size: f64,
    #[serde(rename = "fillSz")]
    pub filled_size: f64,
    pub vwap: f64,
    pub tif: TimeInForce,
    #[serde(rename = "r")]
    pub reduce_only: bool,
    #[serde(rename = "mk")]
    pub maker: bool,
    #[serde(default)]
    pub trigger: Option<TriggerSpec>,
    #[serde(rename = "ts")]
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl OrderState {
    /// Provide side of order
    pub fn side(&self) -> Side {
        if self.signed_size < 0.0 {
            Side::Sell
        } else {
            Side::Buy
        }
    }

    /// Magnitude of size
    pub fn amount(&self) -> f64 {
        self.signed_size.abs()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Fills
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct Fill {
    pub timestamp: u64,
    #[serde(alias = "coin")]
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub price: f64,
    pub size: f64,
    #[serde(rename = "isBuy")]
    pub side: Side,
    #[serde(rename = "maker", default)]
    pub is_maker: bool,
    #[serde(rename = "counterpartyHint", default)]
    pub cpty: String,
    pub reason: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Leverage Setting
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct LeverageSetting {
    #[serde(alias = "coin")]
    pub symbol: String,
    pub leverage: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct CommissionApproval {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub recipient: Pubkey,
    pub max_fee: u8,
}

// ─────────────────────────────────────────────────────────────────────────────
// Full Account (HTTP response)
// ─────────────────────────────────────────────────────────────────────────────

/// The inner payload of a `fullAccount` HTTP response.
///
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(unused)]
pub struct AccountData {
    pub positions: Vec<PositionInfo>,
    pub open_orders: Vec<OrderState>,
    pub margin: Margin,
    pub leverage_settings: Vec<LeverageSetting>,
    #[serde(default, rename = "builderCodeApprovals")]
    pub commission_approvals: Option<Vec<CommissionApproval>>,
}

//
// Unit tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_state_rejected_risk_limit() {
        let json = r#"{
            "ts": 1770918312787284000,
            "ot": "limit",
            "status": "rejectedRiskLimit",
            "sym": "BTC-USD",
            "oid": "EF2bxQ5pp3CDFAwRi44ExXb32sRmByByYxjwLYBfvRKQ",
            "px": 100001.37,
            "origSz": -0.02474,
            "sz": -0.02474,
            "fillSz": 0.0,
            "vwap": 0.0,
            "mk": true,
            "r": false,
            "tif": "gtc",
            "reason": "no oracle / fair price reference yet for: BTC-USD"
        }"#;

        let order: OrderState = serde_json::from_str(json).unwrap();

        assert_eq!(order.symbol, "BTC-USD");
        assert_eq!(
            order.order_id,
            "EF2bxQ5pp3CDFAwRi44ExXb32sRmByByYxjwLYBfvRKQ"
        );
        assert_eq!(order.status, OrderStatus::RejectedRiskLimit);
        assert!(order.signed_size < 0.0);
        assert!((order.price - 100001.37).abs() < 1e-6);
        assert!((order.original_size.abs() - 0.02474).abs() < 1e-8);
        assert!((order.signed_size.abs() - 0.02474).abs() < 1e-8);
        assert_eq!(order.filled_size, 0.0);
        assert!(order.maker);
        assert_eq!(order.timestamp, 1770918312787284000);
        assert_eq!(
            order.reason.as_deref(),
            Some("no oracle / fair price reference yet for: BTC-USD")
        );

        // status helpers
        assert!(order.status.is_terminal());
        assert!(order.status.is_rejected());
    }

    #[test]
    fn margin_deserializes_new_api_fields() {
        let margin: Margin = serde_json::from_str(
            r#"{
                "totalMargin": 771.15,
                "availableMargin": 230.22,
                "executionImpact": -179.22,
                "transferableBalance": 0.0,
                "marginUsed": 540.93,
                "marginBufferRate": 0.05,
                "bufferedMargin": 567.98,
                "notional": 21260.96,
                "realizedPnl": -179.90,
                "unrealizedPnl": -23.72,
                "fees": -42.43,
                "funding": -6.52
            }"#,
        )
        .unwrap();

        assert_eq!(margin.total_margin, 771.15);
        assert_eq!(margin.available_margin, 230.22);
        assert_eq!(margin.execution_impact, -179.22);
        assert_eq!(margin.transferable_balance, 0.0);
        assert_eq!(margin.margin_buffer_rate, 0.05);
        assert_eq!(margin.buffered_margin, 567.98);
    }

    #[test]
    fn margin_accepts_legacy_balance_names() {
        let margin: Margin = serde_json::from_str(
            r#"{
                "totalBalance": 100.0,
                "availableBalance": 90.0,
                "marginUsed": 10.0,
                "notional": 0.0,
                "realizedPnl": 0.0,
                "unrealizedPnl": 0.0,
                "fees": 0.0,
                "funding": 0.0
            }"#,
        )
        .unwrap();

        assert_eq!(margin.total_margin, 100.0);
        assert_eq!(margin.available_margin, 90.0);
        assert_eq!(margin.execution_impact, 0.0);
        assert_eq!(margin.transferable_balance, 0.0);
    }
}
