use solana_pubkey::Pubkey;
use eyre::bail;
use solana_keypair::{keypair_from_seed, Keypair};
use solana_signature::Signature;
use solana_signer::Signer;
// ─────────────────────────────────────────────────────────────────────────────
// Signable trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for any transaction type that can be serialized and signed.
///
/// Implement this for each specialized bundle type
#[allow(unused)]
pub trait Signable {
    /// Serialize to the binary format that gets signed with Ed25519.
    fn serialize(&self) -> eyre::Result<Vec<u8>>;

    /// Store the computed signature.
    fn set_signature(&mut self, sig: Signature);

    /// Retrieve the signature, if present.
    fn get_signature(&self) -> Option<&Signature>;
}


/// Ed25519 signer for Bulk exchange transactions.
///
/// Signs any type that implements [`Signable`]
///
/// # Example
///
/// ```rust,no_run
/// use bulk_api::common::TransactionSigner;
///
/// let signer = TransactionSigner::from_private_key(...)
/// let pk = signer.pubkey();
///
/// let order = LimitOrder::new("BTC-USD", Side::Buy, 98_000.0, 1.0);
/// let mut bundle = OrderBundle::new(vec![order.into()], signer.make_nonce(), pk, pk);
///
/// signer.sign(&mut bundle).unwrap();
/// let ws_json = bundle.to_ws_request(1).unwrap();
/// ```
#[derive(Debug)]
#[allow(unused)]
pub struct TransactionSigner {
    keypair: Keypair
}

#[allow(unused)]
impl TransactionSigner {

    /// Create a signer from a base58-encoded private key (32-byte seed).
    ///
    /// Accepts either a 32-byte seed or a 64-byte expanded key (only the
    /// first 32 bytes are used as the seed).
    pub fn from_private_key(key_b58: &str) -> eyre::Result<Self> {
        let key_bytes = bs58::decode(key_b58)
            .into_vec()?;

        let keypair = if key_bytes.len() == 64 {
            // Full 64-byte keypair (secret + public)
            Keypair::try_from(key_bytes.as_slice())
                .map_err(|e| eyre::eyre!("invalid 64-byte keypair: {e}"))?
        } else if key_bytes.len() >= 32 {
            // 32-byte seed — derive the keypair
            keypair_from_seed(&key_bytes[..32])
                .map_err(|e| eyre::eyre!("failed to create keypair from seed: {e}"))?
        } else {
            bail!("private key {} is wrong size (got {} bytes)", key_b58, key_bytes.len());
        };

        Ok(Self { keypair })
    }

    /// Sign any [`Signable`] transaction in-place.
    ///
    /// Serializes the transaction, signs with Ed25519, and stores
    /// the raw 64-byte signature via [`Signable::set_signature`].
    pub fn sign<T: Signable>(&self, tx: &mut T) -> eyre::Result<()> {
        let message = tx.serialize()?;
        let signature = self.keypair.sign_message(&message);
        tx.set_signature(signature);
        Ok(())
    }

    /// Sign an arbitrary byte slice and return the raw 64-byte signature.
    ///
    /// Used by [`BulkHttpClient`] for generic (non-`Signable`) payloads
    /// such as leverage updates, agent wallet management, and faucet requests,
    /// where the exchange expects a signature over the canonical JSON string.
    pub fn sign_bytes(&self, message: &[u8]) -> Signature {
        self.keypair.sign_message(message)
    }

    /// Get pubkey
    pub fn public_key(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    /// Get pubkey as b58 encoding
    pub fn public_key_b58(&self) -> String {
        self.keypair.pubkey().to_string()
    }
}

impl Clone for TransactionSigner {
    fn clone(&self) -> Self {
        Self {
            keypair: self.keypair.insecure_clone(),
        }
    }
}