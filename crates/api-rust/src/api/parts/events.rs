use serde_json::Value;
use crate::msgs::account::{Fill, LeverageSetting, Margin, OrderState, PositionInfo};
use crate::msgs::md::{Candle, L2Snapshot, Ticker, Trade};


// ─────────────────────────────────────────────────────────────────────────────
// Event topics
// ─────────────────────────────────────────────────────────────────────────────


/// Topics to subscribe to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(unused)]
pub enum Topic {
    Ticker,
    Trades,
    L2Snapshot,
    L2Delta,
    Candle,
    Account,
    Margin,
    Position,
    Fill,
    Leverage,
    Order,
    Error,
    /// Connection lifecycle events (connected, disconnected).
    Status
}

// ─────────────────────────────────────────────────────────────────────────────
// Event return types
// ─────────────────────────────────────────────────────────────────────────────

/// Typed event payload emitted to handlers.
#[derive(Debug, Clone)]
#[allow(unused)]
pub enum Event {
    Ticker(Ticker),
    Trades(Vec<Trade>),
    L2Snapshot(L2Snapshot),
    L2Delta(L2Snapshot),
    Candle(Candle),
    Margin(Margin),
    Position(PositionInfo),
    Order(OrderState),
    Fill(Fill),
    Leverage(Vec<LeverageSetting>),
    Error(Value),
    /// Emitted on the [`Topic::Status`] channel when the WebSocket connection
    /// is cleanly established (after the actor sends initial subscriptions).
    Connected,
    /// Emitted on the [`Topic::Status`] channel when the WebSocket connection
    /// is lost for any reason.  The string contains a human-readable cause.
    Disconnected(String),
}

#[allow(unused)]
impl Event {
    pub fn topic(&self) -> Topic {
        match self {
            Event::Ticker(_) => Topic::Ticker,
            Event::Trades(_) => Topic::Trades,
            Event::L2Snapshot(_) => Topic::L2Snapshot,
            Event::L2Delta(_) => Topic::L2Delta,
            Event::Candle(_) => Topic::Candle,
            Event::Margin(_) => Topic::Margin,
            Event::Position(_) => Topic::Position,
            Event::Order(_) => Topic::Order,
            Event::Fill(_) => Topic::Fill,
            Event::Leverage(_) => Topic::Leverage,
            Event::Error(_) => Topic::Error,
            Event::Connected | Event::Disconnected(_) => Topic::Status,
        }
    }
}