use std::fmt::Write;
use std::fmt;
use solana_pubkey::Pubkey;
use crate::common::{write_f64, write_pubkey_bytes, write_string_u64, write_u32, write_u64, Signable};
use crate::msgs::oracle::OraclePrice;

/// A bundle of oracle price updates that forms a single signed transaction.
///
/// ## Wire format (all little-endian)
///
/// ```text
/// action_code    : u32       = 1 (oracle)
/// num_prices     : u64
/// for each price:
///   timestamp    : u64
///   symbol       : u64-prefixed string
///   price        : f64
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
/// let mut bundle = OracleBundle::new(
///     vec![
///         OraclePrice::new(1700000000000, "BTC-USD", 98_000.0),
///         OraclePrice::new(1700000000000, "ETH-USD", 3_200.0),
///     ],
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
pub struct OracleTransaction {
    pub prices: Vec<OraclePrice>,
    pub nonce: u64,
    pub account: Pubkey,
    pub signer: Pubkey,
    pub signature: Option<[u8; 64]>,
}

#[allow(unused)]
impl OracleTransaction {
    /// Create a new unsigned oracle bundle.
    pub fn new(
        prices: Vec<OraclePrice>,
        nonce: u64,
        account: Pubkey,
        signer: Pubkey,
    ) -> Self {
        Self {
            prices,
            nonce,
            account,
            signer,
            signature: None,
        }
    }

    /// Push an additional price update into the bundle.
    pub fn push(&mut self, price: OraclePrice) {
        self.signature = None; // invalidate any existing signature
        self.prices.push(price);
    }

    /// Number of price updates in the bundle.
    pub fn len(&self) -> usize {
        self.prices.len()
    }

    /// Whether the bundle is empty.
    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
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
    ///     "type": "oracle",
    ///     "oracles": [ {"t": ..., "c": ..., "px": ...}, ... ],
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

        let mut buf = String::with_capacity(self.prices.len() * 60 + 256);

        write!(buf,
               r#"{{"action":{{"type":"oracle","oracles":["#
        ).unwrap();

        for (i, price) in self.prices.iter().enumerate() {
            if i > 0 { buf.push(','); }
            price.write_api(&mut buf);
        }

        write!(buf,
               r#"],"nonce":{}}},"account":"{}","signer":"{}","signature":"{}"}}"#,
               self.nonce, saccount, ssigner, sig
        ).unwrap();

        Ok(buf)
    }

    /// Wrap the signed bundle in the WebSocket request envelope.
    pub fn to_ws_request_string(&self, request_id: u64) -> eyre::Result<String> {
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

impl Signable for OracleTransaction {
    fn serialize(&self) -> eyre::Result<Vec<u8>> {
        /// Action code for oracle updates (must match backend).
        const ACTION_ORACLE: u32 = 1;

        let mut buf = Vec::with_capacity(128);

        // 1. action code
        write_u32(&mut buf, ACTION_ORACLE);

        // 2. number of price updates + each entry
        write_u64(&mut buf, self.prices.len() as u64);
        for price in &self.prices {
            write_u64(&mut buf, price.timestamp);
            write_string_u64(&mut buf, &price.symbol);
            write_f64(&mut buf, price.price);
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

impl fmt::Display for OracleTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OracleBundle({:?})",
            self.prices
        )
    }
}