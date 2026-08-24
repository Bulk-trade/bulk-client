use serde::Deserialize;
use serde_json::Value;

// ─────────────────────────────────────────────────────────────────────────────
// Order responses (mirrors trade.py OrderResponse)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[allow(unused)]
pub struct Response {
    pub order_id: Option<String>,
    pub status: String,
    pub message: Option<String>,
    #[serde(skip)]
    pub raw: Value,
}

#[allow(unused)]
impl Response {
    /// Indicate whether response is an error
    pub fn is_error(&self) -> bool {
        matches!(
            self.status.as_str(),
            "error"
                | "rejectedRiskLimit"
                | "rejectedInvalid"
                | "rejectedDuplicate"
                | "rejectedCrossing"
                | "multisigCreatedFailed"
                | "proposalFailed"
                | "proposalRejected"
        )
    }

    /// Indicate whether response is a placement
    pub fn is_placement(&self) -> bool {
        matches!(self.status.as_str(), "resting" | "working" | "filled")
    }

    /// Parse the list of statuses from a post response (same logic as Python).
    pub(crate) fn parse_responses(data: &Value) -> Vec<Self> {
        // WS: data.data.payload.response.data.statuses
        // HTTP: data.response.data.statuses
        let statuses = data["data"]["payload"]["response"]["data"]["statuses"]
            .as_array()
            .or_else(|| data["response"]["data"]["statuses"].as_array());

        let Some(arr) = statuses else {
            return vec![];
        };

        arr.iter()
            .map(|entry| {
                // Each entry is an externally tagged status such as
                // {"resting": {...}} or {"proposalFailed": {...}}.
                let status_key = entry
                    .as_object()
                    .and_then(|map| map.keys().next())
                    .cloned()
                    .unwrap_or_default();
                let body = &entry[&status_key];
                Response {
                    order_id: body["oid"].as_str().map(Into::into),
                    status: status_key,
                    message: body["message"]
                        .as_str()
                        .or_else(|| body["error"].as_str())
                        .or_else(|| body["reason"].as_str())
                        .map(Into::into),
                    raw: body.clone(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_proposal_failure_message_as_error() {
        let data = json!({
            "response": {
                "data": {
                    "statuses": [{
                        "proposalFailed": {
                            "proposalId": 7,
                            "status": "failed",
                            "message": "embedded action failed"
                        }
                    }]
                }
            }
        });

        let responses = Response::parse_responses(&data);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].status, "proposalFailed");
        assert_eq!(
            responses[0].message.as_deref(),
            Some("embedded action failed")
        );
        assert!(responses[0].is_error());
    }

    #[test]
    fn preserves_placement_parsing_without_message() {
        let data = json!({
            "response": {
                "data": {
                    "statuses": [{"resting": {"oid": "order-id"}}]
                }
            }
        });

        let responses = Response::parse_responses(&data);

        assert_eq!(responses[0].status, "resting");
        assert_eq!(responses[0].order_id.as_deref(), Some("order-id"));
        assert_eq!(responses[0].message, None);
        assert!(responses[0].is_placement());
    }

    #[test]
    fn parses_multisig_rejection_error() {
        let data = json!({
            "response": {
                "data": {
                    "statuses": [{
                        "proposalRejected": {
                            "proposalId": 0,
                            "error": "multisig policy does not match configured admin policy"
                        }
                    }]
                }
            }
        });

        let responses = Response::parse_responses(&data);

        assert_eq!(
            responses[0].message.as_deref(),
            Some("multisig policy does not match configured admin policy")
        );
    }

    #[test]
    fn parses_transaction_rejection_reason() {
        let data = json!({
            "response": {
                "data": {
                    "statuses": [{"transactionRejected": {"reason": "expired"}}]
                }
            }
        });

        let responses = Response::parse_responses(&data);

        assert_eq!(responses[0].message.as_deref(), Some("expired"));
    }
}
