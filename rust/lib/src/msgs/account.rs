use serde::Deserialize;
use crate::common::order_status::OrderStatus;
use crate::common::side::Side;

// ─────────────────────────────────────────────────────────────────────────────
// Margin
// ─────────────────────────────────────────────────────────────────────────────

/// Account margin information
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(unused)]
pub struct Margin {
    #[serde(rename="totalBalance")]
    pub total_balance: f64,
    #[serde(rename="availableBalance")]
    pub available_balance: f64,
    #[serde(rename="marginUsed")]
    pub margin_used: f64,
    pub notional: f64,
    #[serde(rename="realizedPnl")]
    pub realized_pnl: f64,
    #[serde(rename="unrealizedPnl")]
    pub unrealized_pnl: f64,
    pub fees: f64,
    pub funding: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Positions
// ─────────────────────────────────────────────────────────────────────────────

/// Information for a position
#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct PositionInfo {
    pub symbol: String,
    pub size: f64,
    pub price: f64,
    #[serde(rename="fairPrice")]
    pub fair_price: f64,
    pub notional: f64,
    #[serde(rename="realizedPnl")]
    pub realized_pnl: f64,
    #[serde(rename="unrealizedPnl")]
    pub unrealized_pnl: f64,
    pub leverage: f64,
    #[serde(rename="liquidationPrice")]
    pub liquidation_price: f64,
    #[serde(rename="maintenanceMargin")]
    pub maintenance_margin: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Order State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone,Deserialize)]
#[allow(unused)]
pub struct OrderState {
    pub timestamp: u64,
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub status: OrderStatus,
    #[serde(rename = "isBuy")]
    pub side: Side,
    pub price: f64,
    pub size: f64,
    #[serde(rename = "filledSize")]
    pub filled_size: f64,
    #[serde(rename = "originalSize")]
    pub original_size: f64,
    #[serde(rename = "maker")]
    pub is_maker: bool,
    #[serde(rename = "reason", default)]
    pub error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Fills
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct Fill {
    pub timestamp: u64,
    pub symbol: String,
    #[serde(rename="orderId")]
    pub order_id: String,
    pub price: f64,
    pub size: f64,
    #[serde(rename="isBuy")]
    pub side: Side,
    #[serde(rename="maker")]
    pub is_maker: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Leverage Setting
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct LeverageSetting {
    pub symbol: String,
    pub leverage: f64,
}