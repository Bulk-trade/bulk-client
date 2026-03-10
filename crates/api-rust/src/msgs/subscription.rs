use serde_json::Value;

/// Subscription request message
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct SubscriptionRequest {
    sub_type: String,
    params: Value,
}

#[allow(unused)]
impl SubscriptionRequest {
    pub(crate) fn new(sub_type: impl Into<String>, params: Value) -> Self {
        Self {
            sub_type: sub_type.into(),
            params,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("type".into(), Value::String(self.sub_type.clone()));
        if let Value::Object(map) = &self.params {
            for (k, v) in map {
                obj.insert(k.clone(), v.clone());
            }
        }
        Value::Object(obj)
    }
}