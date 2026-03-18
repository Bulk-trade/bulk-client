use serde::{Deserialize, Serialize};
use crate::transaction::ActionMeta;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Price {
    #[serde(rename = "t")]
    pub timestamp: u64,
    #[serde(rename = "c")]
    pub asset: String,
    #[serde(rename = "px")]
    pub price: f64,

    #[serde(skip)]
    pub meta: ActionMeta,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PythPrice {
    #[serde(rename = "t")]
    pub timestamp: u64,
    #[serde(rename = "fi")]
    pub id: u64,
    #[serde(rename = "px")]
    pub px: u64,
    #[serde(rename = "e")]
    pub exponent: i16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PythOracle {
    pub oracles: Vec<PythPrice>,

    #[serde(skip)]
    pub meta: ActionMeta,
}
