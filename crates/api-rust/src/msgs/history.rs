use {
    reqwest::StatusCode,
    serde::{de, Deserialize, Deserializer, Serialize, Serializer},
    solana_hash::Hash,
    solana_pubkey::Pubkey,
    std::{error::Error, fmt, str::FromStr},
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_slot: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_slot: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage<T> {
    pub data: Vec<T>,
    pub page: HistoryPageInfo,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPageInfo {
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub as_of_slot: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    pub coverage: HistoryCoverageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_available_slot: Option<u64>,
}

impl<'de> Deserialize<'de> for HistoryPageInfo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            #[serde(default, deserialize_with = "deserialize_present_nullable")]
            next_cursor: Option<Option<String>>,
            has_more: bool,
            as_of_slot: u64,
            start_slot: u64,
            end_slot: u64,
            coverage: HistoryCoverageStatus,
            #[serde(default)]
            min_available_slot: Option<u64>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let next_cursor = wire
            .next_cursor
            .ok_or_else(|| de::Error::missing_field("nextCursor"))?;
        if wire.has_more != next_cursor.is_some()
            || next_cursor.as_ref().is_some_and(String::is_empty)
        {
            return Err(de::Error::custom(
                "history page hasMore and nextCursor are inconsistent",
            ));
        }
        if wire.start_slot > wire.end_slot {
            return Err(de::Error::custom(
                "history page startSlot must not exceed endSlot",
            ));
        }
        if wire.end_slot > wire.as_of_slot {
            return Err(de::Error::custom(
                "history page endSlot must not exceed asOfSlot",
            ));
        }
        if wire.coverage == HistoryCoverageStatus::Complete
            && wire
                .min_available_slot
                .is_some_and(|slot| slot > wire.start_slot)
        {
            return Err(de::Error::custom(
                "history page minAvailableSlot must not exceed startSlot when coverage is complete",
            ));
        }

        Ok(Self {
            next_cursor,
            has_more: wire.has_more,
            as_of_slot: wire.as_of_slot,
            start_slot: wire.start_slot,
            end_slot: wire.end_slot,
            coverage: wire.coverage,
            min_available_slot: wire.min_available_slot,
        })
    }
}

fn deserialize_present_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryCoverageStatus {
    Complete,
    Partial,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryErrorEnvelope {
    pub error: HistoryErrorBody,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum HistoryHttpError {
    Transport(reqwest::Error),
    Api {
        status: StatusCode,
        body: HistoryErrorEnvelope,
    },
}

impl fmt::Display for HistoryHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => write!(formatter, "history HTTP transport error: {error}"),
            Self::Api { status, body } => write!(
                formatter,
                "history API returned {status}: {}: {}",
                body.error.code, body.error.message
            ),
        }
    }
}

impl Error for HistoryHttpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Api { .. } => None,
        }
    }
}

impl From<reqwest::Error> for HistoryHttpError {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TradeId {
    pub slot: u64,
    pub sequence: u64,
}

impl fmt::Display for TradeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.slot, self.sequence)
    }
}

impl Serialize for TradeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TradeId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let (slot, sequence) = value
            .split_once(':')
            .filter(|(slot, sequence)| {
                !slot.is_empty()
                    && !sequence.is_empty()
                    && slot.bytes().all(|byte| byte.is_ascii_digit())
                    && sequence.bytes().all(|byte| byte.is_ascii_digit())
                    && (*slot == "0" || !slot.starts_with('0'))
                    && (*sequence == "0" || !sequence.starts_with('0'))
                    && !sequence.contains(':')
            })
            .ok_or_else(|| de::Error::custom("tradeId must be <slot>:<sequence>"))?;
        Ok(Self {
            slot: slot.parse().map_err(de::Error::custom)?,
            sequence: sequence.parse().map_err(de::Error::custom)?,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFill {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub maker: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub taker: Pubkey,
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id_maker: Hash,
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id_taker: Hash,
    pub trade_id: TradeId,
    pub is_buy: bool,
    pub symbol: String,
    pub amount: f64,
    pub price: f64,
    pub maker_fee: f64,
    pub taker_fee: f64,
    pub fee: f64,
    pub reason_code: u8,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterparty_hint: Option<String>,
    pub slot: u64,
    pub timestamp: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPosition {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub owner: Pubkey,
    pub symbol: String,
    #[serde(default)]
    pub quantity: f64,
    #[serde(default)]
    pub max_quantity: f64,
    pub total_volume: f64,
    pub avg_open_price: f64,
    pub avg_close_price: f64,
    pub realized_pnl: f64,
    pub fees: f64,
    pub funding: f64,
    pub open_time: u64,
    pub close_time: u64,
    pub close_reason: String,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    pub close_slot: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingPayment {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub owner: Pubkey,
    pub symbol: String,
    pub size: f64,
    pub payment: f64,
    pub funding_rate: f64,
    pub mark_price: f64,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    pub slot: u64,
    pub timestamp: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryTrigger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_above: Option<bool>,
    pub px: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lim: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "option_hash")]
    pub oco: Option<Hash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub px_hi: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lim_hi: Option<f64>,
    #[serde(rename = "trb", default, skip_serializing_if = "Option::is_none")]
    pub trail_bps: Option<u32>,
    #[serde(rename = "stb", default, skip_serializing_if = "Option::is_none")]
    pub step_bps: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOrder {
    #[serde(with = "crate::msgs::serde_hash")]
    pub order_id: Hash,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub tif: String,
    pub price: f64,
    pub vwap: f64,
    pub original_size: f64,
    pub executed_size: f64,
    pub reduce_only: bool,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<HistoryTrigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    pub slot: u64,
    pub timestamp: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountActivity {
    pub activity_type: String,
    pub status: String,
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub from: Pubkey,
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub to: Pubkey,
    pub symbol: String,
    pub amount: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    pub slot: u64,
    pub timestamp: u64,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskEventType {
    Liquidation,
    Adl,
    RiskVault,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskEvent {
    #[serde(with = "crate::msgs::serde_pubkey")]
    pub owner: Pubkey,
    pub symbol: String,
    pub is_buy: bool,
    pub amount: f64,
    pub price: f64,
    pub event_type: RiskEventType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_prior: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_after: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub iso: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "option_pubkey"
    )]
    pub iso_pubkey: Option<Pubkey>,
    pub slot: u64,
    pub timestamp: u64,
    pub sequence: u64,
}

mod option_pubkey {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        value: &Option<Pubkey>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Pubkey>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|value| Pubkey::from_str(&value).map_err(serde::de::Error::custom))
            .transpose()
    }
}

mod option_hash {
    use super::*;

    pub fn serialize<S: serde::Serializer>(
        value: &Option<Hash>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Hash>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                Hash::from_str(&value).map_err(|error| {
                    serde::de::Error::custom(format!("invalid base58 hash: {error}"))
                })
            })
            .transpose()
    }
}
