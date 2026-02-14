use std::fmt::Write;
use std::fmt;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use crate::common::{write_pubkey_bytes, write_u32, write_u64, Signable};
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
    pub fn write_api(&self, buf: &mut String) {
        match self {
            OrderAction::Limit(o) => o.write_api(buf),
            OrderAction::Market(o) => o.write_api(buf),
            OrderAction::Cancel(o) => o.write_api(buf),
            OrderAction::CancelAll(o) => o.write_api(buf),
        }
    }

    /// Set nonce and pubkey context for the message
    ///
    /// # Arguments
    /// - `nonce`: nonce to be used
    /// - `account`: the account to be used
    pub fn set_context(&mut self, nonce: u64, account: Pubkey) {
        match self {
            OrderAction::Limit(o) => {
                o.nonce = Some(nonce);
                o.pubkey = Some(account);
            }
            OrderAction::Market(o) => {
                o.nonce = Some(nonce);
                o.pubkey = Some(account);
            },
            _ => {}
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
            OrderAction::Cancel(o) => {
                o.serialize_for_tx(buf)
            },
            OrderAction::CancelAll(o) => {
                o.serialize_for_tx(buf)?;
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
#[allow(unused)]
pub struct OrderTransaction {
    pub actions: Vec<OrderAction>,
    pub nonce: u64,
    pub account: Pubkey,
    pub signer: Pubkey,
    pub signature: Option<Signature>,
}

#[allow(unused)]
impl OrderTransaction {
    /// Create a new unsigned order bundle.
    pub fn new(
        actions: Vec<OrderAction>,
        nonce: u64,
        account: Pubkey,
        signer: Pubkey,
    ) -> Self {
        // attach nonce and account to tx
        let actions = actions.iter()
            .map(|a| {
                let mut o = a.clone();
                o.set_context(nonce, account.clone());
                o
            })
            .collect();

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
        // invalidate any existing signature
        self.signature = None;

        // attach nonce and account to tx
        let mut action = action.into();
        action.set_context(self.nonce, self.account.clone());

        self.actions.push(action);
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
    pub fn to_api_string(&self) -> eyre::Result<String> {
        let sig = self.signature_b58()
            .ok_or_else(|| eyre::eyre!("Missing signature"))?;

        let saccount = bs58::encode(&self.account).into_string();
        let ssigner = bs58::encode(&self.signer).into_string();

        // Pre-size: ~80 bytes per order + envelope overhead
        let mut buf = String::with_capacity(self.actions.len() * 100 + 256);

        write!(buf,
               r#"{{"action":{{"type":"order","orders":["#
        ).unwrap();

        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 { buf.push(','); }
            action.write_api(&mut buf);
        }

        write!(buf,
               r#"],"nonce":{}}},"account":"{}","signer":"{}","signature":"{}"}}"#,
               self.nonce, saccount, ssigner, sig
        ).unwrap();

        Ok(buf)
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
    pub fn to_ws_request(&self, request_id: u64) -> eyre::Result<String> {
        let payload = self.to_api_string()?;

        let mut buf = String::with_capacity(payload.len() + 80);
        write!(
            buf,
            r#"{{"method":"post","request":{{"type":"action","payload":{}}},"id":{}}}"#,
            payload, request_id
        ).unwrap();

        Ok(buf)
    }
}

impl Signable for OrderTransaction {
    fn serialize(&self) -> eyre::Result<Vec<u8>> {
        /// Action code for order bundles (must match backend).
        const ACTION_ORDER: u32 = 0;

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

    fn set_signature(&mut self, sig: Signature) {
        self.signature = Some(sig);
    }

    fn get_signature(&self) -> Option<&Signature> {
        self.signature.as_ref()
    }
}

impl fmt::Display for OrderTransaction {
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
