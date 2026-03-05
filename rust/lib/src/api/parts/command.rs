// ═════════════════════════════════════════════════════════════════════════════
// Commands (client handle → actor)
// ═════════════════════════════════════════════════════════════════════════════

use tokio::sync::oneshot;
use crate::api::parts::{Event};
use crate::msgs::account::OrderState;
use crate::msgs::responses::Response;
use crate::msgs::subscription::SubscriptionRequest;

/// User-supplied callback. Receives the raw JSON payload for the topic.
/// Runs synchronously inside the actor loop — keep it fast or spawn.
#[allow(unused)]
type EventHandler = Box<dyn Fn(&Event) + Send + Sync>;

pub(crate) enum Command {
    /// Subscribe to additional topics.
    Subscribe(Vec<SubscriptionRequest>),

    /// Send a signed order/cancel payload through the WebSocket
    /// and wait for the post response.
    Tx {
        request_id: u64,
        json: String,
        respond: oneshot::Sender<eyre::Result<Vec<Response>>>,
    },

    /// Send a raw JSON message (e.g. oracle update, fire-and-forget).
    SendRaw(String),

    /// Query open orders (cold path — goes through actor).
    GetOrders {
        symbol: Option<String>,
        respond: oneshot::Sender<Vec<OrderState>>,
    },

    /// Graceful shutdown.
    Shutdown,
}