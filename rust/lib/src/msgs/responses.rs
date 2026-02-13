use serde::{Deserialize};
use serde_json::Value;

// ─────────────────────────────────────────────────────────────────────────────
// Order responses (mirrors trade.py OrderResponse)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct OrderResponse {
    pub order_id: Option<String>,
    pub status: String,
    pub message: Option<String>,
    #[serde(skip)]
    pub raw: Value,
}

#[allow(unused)]
impl OrderResponse {
    pub fn is_error(&self) -> bool {
        matches!(
            self.status.as_str(),
            "error"
                | "rejectedRiskLimit"
                | "rejectedInvalid"
                | "rejectedDuplicate"
                | "rejectedCrossing"
        )
    }

    /// Parse the list of statuses from a post response (same logic as Python).
    pub(crate) fn parse_responses(data: &Value) -> Vec<Self> {
        let statuses = &data["data"]["payload"]["response"]["data"]["statuses"];
        let Some(arr) = statuses.as_array() else {
            return vec![];
        };

        arr.iter()
            .map(|entry| {
                if let Some(body) = entry.get("error") {
                    OrderResponse {
                        order_id: None,
                        status: "error".into(),
                        message: body["message"].as_str().map(Into::into),
                        raw: body.clone(),
                    }
                } else {
                    // First key is the status string
                    let status_key = entry
                        .as_object()
                        .and_then(|m| m.keys().next())
                        .unwrap_or(&String::new())
                        .clone();
                    let body = &entry[&status_key];
                    OrderResponse {
                        order_id: body["oid"].as_str().map(Into::into),
                        status: status_key,
                        message: None,
                        raw: body.clone(),
                    }
                }
            })
            .collect()
    }
}
