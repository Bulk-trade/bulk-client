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

//
// Unit tests
//

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_state_rejected_risk_limit() {
        let json = r#"{
            "status": "rejectedRiskLimit",
            "symbol": "BTC-USD",
            "orderId": "EF2bxQ5pp3CDFAwRi44ExXb32sRmByByYxjwLYBfvRKQ",
            "price": 100001.37,
            "originalSize": 0.02474,
            "size": 0.02474,
            "filledSize": 0.0,
            "vwap": 0.0,
            "isBuy": false,
            "maker": true,
            "tif": "gtc",
            "timestamp": 1770918312787284000,
            "reason": "no oracle / fair price reference yet for: BTC-USD"
        }"#;

        let order: OrderState = serde_json::from_str(json).unwrap();

        assert_eq!(order.symbol, "BTC-USD");
        assert_eq!(order.order_id, "EF2bxQ5pp3CDFAwRi44ExXb32sRmByByYxjwLYBfvRKQ");
        assert_eq!(order.status, OrderStatus::RejectedRiskLimit);
        assert_eq!(order.side, Side::Sell);
        assert!((order.price - 100001.37).abs() < 1e-6);
        assert!((order.original_size - 0.02474).abs() < 1e-8);
        assert!((order.size - 0.02474).abs() < 1e-8);
        assert_eq!(order.filled_size, 0.0);
        assert!(order.is_maker);
        assert_eq!(order.timestamp, 1770918312787284000);
        assert_eq!(
            order.error.as_deref(),
            Some("no oracle / fair price reference yet for: BTC-USD")
        );

        // status helpers
        assert!(order.status.is_terminal());
        assert!(order.status.is_rejected());
    }

    #[test]
    fn test_order_state_from_ws_envelope() {
        let json = r#"{
            "type": "account",
            "data": {
                "type": "orderUpdate",
                "status": "rejectedRiskLimit",
                "symbol": "BTC-USD",
                "orderId": "EF2bxQ5pp3CDFAwRi44ExXb32sRmByByYxjwLYBfvRKQ",
                "price": 100001.37,
                "originalSize": 0.02474,
                "size": 0.02474,
                "filledSize": 0.0,
                "vwap": 0.0,
                "isBuy": false,
                "maker": true,
                "tif": "gtc",
                "timestamp": 1770918312787284000,
                "reason": "no oracle / fair price reference yet for: BTC-USD"
            },
            "topic": "account.2bZfxVQtWdd8qAWJ4Xyq43cnej9zqMNyuh7HHxTNan8j"
        }"#;

        // Parse the same way the actor does: from data["data"]
        let envelope: serde_json::Value = serde_json::from_str(json).unwrap();
        let order: OrderState =
            serde_json::from_value(envelope["data"].clone()).unwrap();

        assert_eq!(order.symbol, "BTC-USD");
        assert_eq!(order.status, OrderStatus::RejectedRiskLimit);
        assert_eq!(order.side, Side::Sell);
        assert!(order.error.is_some());
    }

    #[test]
    fn test_order_state_resting_no_reason() {
        let json = r#"{
            "status": "resting",
            "symbol": "ETH-USD",
            "orderId": "abc123",
            "price": 3200.0,
            "originalSize": 1.0,
            "size": 1.0,
            "filledSize": 0.0,
            "isBuy": true,
            "maker": true,
            "timestamp": 1770918312787284000
        }"#;

        let order: OrderState = serde_json::from_str(json).unwrap();

        assert_eq!(order.status, OrderStatus::Resting);
        assert_eq!(order.side, Side::Buy);
        assert!(order.error.is_none());

        assert!(!order.status.is_terminal());
        assert!(!order.status.is_rejected());
    }

    #[test]
    fn test_order_state_filled() {
        let json = r#"{
            "status": "filled",
            "symbol": "BTC-USD",
            "orderId": "xyz789",
            "price": 98000.0,
            "originalSize": 0.5,
            "size": 0.0,
            "filledSize": 0.5,
            "isBuy": true,
            "maker": false,
            "timestamp": 1770918312787284000
        }"#;

        let order: OrderState = serde_json::from_str(json).unwrap();

        assert_eq!(order.status, OrderStatus::Filled);
        assert!(order.status.is_terminal());
        assert!(!order.status.is_rejected());
        assert!(!order.is_maker);
        assert_eq!(order.filled_size, 0.5);
        assert_eq!(order.size, 0.0);
    }
}