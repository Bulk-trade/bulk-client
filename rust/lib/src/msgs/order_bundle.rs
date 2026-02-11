use std::fmt;
use solana_pubkey::Pubkey;
use crate::msgs::cancel_all::CancelAll;
use crate::msgs::cancel_order::CancelOrder;
use crate::msgs::limit_order::LimitOrder;
use crate::msgs::market_order::MarketOrder;

// ─────────────────────────────────────────────────────────────────────────────
// OrderAction  —  a single item inside a bundled transaction
// ─────────────────────────────────────────────────────────────────────────────

/// Any action that can appear inside an order-bundle transaction.
#[derive(Debug, Clone)]
pub enum OrderAction {
    Limit(LimitOrder),
    Market(MarketOrder),
    Cancel(CancelOrder),
    CancelAll(CancelAll),
}

impl OrderAction {
    /// Produce the JSON fragment for this action.
    pub fn to_api(&self) -> serde_json::Value {
        match self {
            OrderAction::Limit(o) => o.to_api(),
            OrderAction::Market(o) => o.to_api(),
            OrderAction::Cancel(o) => o.to_api(),
            OrderAction::CancelAll(o) => o.to_api(),
        }
    }

    /// Append the binary serialization (transaction / signing format) to `buf`.
    pub fn serialize_for_tx(&self, buf: &mut Vec<u8>) -> eyre::Result<()> {
        match self {
            OrderAction::Limit(o) => {
                o.serialize_for_tx(buf);
                Ok(())
            }
            OrderAction::Market(o) => {
                o.serialize_for_tx(buf);
                Ok(())
            }
            OrderAction::Cancel(o) => o.serialize_for_tx(buf),
            OrderAction::CancelAll(o) => {
                o.serialize_for_tx(buf);
                Ok(())
            }
        }
    }
}

impl fmt::Display for OrderAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderAction::Limit(o) => write!(f, "{o}"),
            OrderAction::Market(o) => write!(f, "{o}"),
            OrderAction::Cancel(o) => write!(f, "{o}"),
            OrderAction::CancelAll(o) => write!(f, "{o}"),
        }
    }
}

// Convenient conversions
impl From<LimitOrder> for OrderAction {
    fn from(o: LimitOrder) -> Self {
        OrderAction::Limit(o)
    }
}
impl From<MarketOrder> for OrderAction {
    fn from(o: MarketOrder) -> Self {
        OrderAction::Market(o)
    }
}
impl From<CancelOrder> for OrderAction {
    fn from(o: CancelOrder) -> Self {
        OrderAction::Cancel(o)
    }
}
impl From<CancelAll> for OrderAction {
    fn from(o: CancelAll) -> Self {
        OrderAction::CancelAll(o)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OrderBundle  —  a bundle of order actions
// ─────────────────────────────────────────────────────────────────────────────

/// A bundle of order-related actions (place, cancel, cancel-all) that
/// forms a single signed transaction.
///
/// ## Wire format (all little-endian)
///
/// ```text
/// action_code    : u32       = 0 (order)
/// num_actions    : u64
/// actions[..]    : variable  (each prefixed by its own order-type tag)
/// nonce          : u64
/// account_pubkey : [u8; 32]
/// signer_pubkey  : [u8; 32]
/// ```
///
/// ## Example
///
/// ```rust,no_run
/// use bulk_sdk::*;
///
/// let signer = TransactionSigner::generate();
/// let pk = signer.pubkey();
///
/// let order = LimitOrder::new("BTC-USD", Side::Buy, 98_000.0, 1.0);
/// let cancel = CancelAll::new(vec!["ETH-USD".into()]);
///
/// let mut bundle = OrderBundle::new(
///     vec![order.into(), cancel.into()],
///     signer.make_nonce(),
///     pk,
///     pk,
/// );
///
/// signer.sign(&mut bundle).unwrap();
/// let ws_json = bundle.to_ws_request(1).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct OrderBundle {
    pub actions: Vec<OrderAction>,
    pub nonce: u64,
    pub account: Pubkey,
    pub signer: Pubkey,
    pub signature: Option<[u8; 64]>,
}

impl OrderBundle {
    /// Create a new unsigned order bundle.
    pub fn new(
        actions: Vec<OrderAction>,
        nonce: u64,
        account: Pubkey,
        signer: Pubkey,
    ) -> Self {
        Self {
            actions,
            nonce,
            account,
            signer,
            signature: None,
        }
    }

    /// Push an additional action into the bundle.
    pub fn push(&mut self, action: impl Into<OrderAction>) {
        self.signature = None; // invalidate any existing signature
        self.actions.push(action.into());
    }

    /// Number of actions in the bundle.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Whether the bundle is empty.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Whether the bundle has been signed.
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }

    /// Base58-encoded signature string, or `None` if unsigned.
    pub fn signature_b58(&self) -> Option<String> {
        self.signature.map(|s| bs58::encode(s).into_string())
    }

    /// Build the full JSON payload for the signed transaction.
    ///
    /// ```json
    /// {
    ///   "action": {
    ///     "type": "order",
    ///     "orders": [ ... ],
    ///     "nonce": 123456789
    ///   },
    ///   "account": "...",
    ///   "signer": "...",
    ///   "signature": "..."
    /// }
    /// ```
    pub fn to_api(&self) -> eyre::Result<serde_json::Value> {
        let sig = self.signature_b58()
            .ok_or_else(|| eyre::eyre!("Missing signature"))?;

        let orders: Vec<_> = self.actions.iter().map(|a| a.to_api()).collect();

        let saccount = bs58::encode(&self.account).into_string();
        let ssigner = bs58::encode(&self.signer).into_string();

        Ok(serde_json::json!({
            "action": {
                "type": "order",
                "orders": orders,
                "nonce": self.nonce,
            },
            "account": saccount,
            "signer": ssigner,
            "signature": sig,
        }))
    }

    /// Wrap the signed bundle in the WebSocket request envelope.
    ///
    /// ```json
    /// {
    ///   "method": "post",
    ///   "request": { "type": "action", "payload": { ... } },
    ///   "id": 1
    /// }
    /// ```
    pub fn to_ws_request(&self, request_id: u64) -> eyre::Result<serde_json::Value> {
        let payload = self.to_api()?;
        Ok(serde_json::json!({
            "method": "post",
            "request": {
                "type": "action",
                "payload": payload,
            },
            "id": request_id,
        }))
    }
}

impl Signable for OrderBundle {
    fn serialize(&self) -> eyre::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(256);

        // 1. action code
        write_u32(&mut buf, ACTION_ORDER);

        // 2. number of actions + each action's serialized body
        write_u64(&mut buf, self.actions.len() as u64);
        for action in &self.actions {
            action.serialize_for_tx(&mut buf)?;
        }

        // 3. nonce
        write_u64(&mut buf, self.nonce);

        // 4. account + signer public keys (raw 32 bytes each)
        write_pubkey_bytes(&mut buf, &self.account);
        write_pubkey_bytes(&mut buf, &self.signer);

        Ok(buf)
    }

    fn set_signature(&mut self, sig: [u8; 64]) {
        self.signature = Some(sig);
    }

    fn get_signature(&self) -> Option<&[u8; 64]> {
        self.signature.as_ref()
    }
}

impl fmt::Display for OrderBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OrderBundle({} actions, nonce={}, signed={})",
            self.actions.len(),
            self.nonce,
            self.is_signed()
        )
    }
}
