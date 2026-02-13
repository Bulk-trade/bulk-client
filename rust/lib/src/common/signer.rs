use solana_pubkey::Pubkey;
use ed25519_dalek::{Signer as _, SigningKey, VerifyingKey};
use eyre::bail;
// ─────────────────────────────────────────────────────────────────────────────
// Signable trait
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for any transaction type that can be serialized and signed.
///
/// Implement this for each specialized bundle type (e.g. `OrderBundle`,
/// a future `OracleBundle`, etc.) so the [`TransactionSigner`](crate::TransactionSigner)
/// can sign them uniformly.
#[allow(unused)]
pub trait Signable {
    /// Serialize to the binary format that gets signed with Ed25519.
    fn serialize(&self) -> eyre::Result<Vec<u8>>;

    /// Store the computed signature.
    fn set_signature(&mut self, sig: [u8; 64]);

    /// Retrieve the signature, if present.
    fn get_signature(&self) -> Option<&[u8; 64]>;
}


/// Ed25519 signer for Bulk exchange transactions.
///
/// Signs any type that implements [`Signable`] — currently [`OrderBundle`](crate::OrderBundle),
/// but the trait is open for future bundle types (oracle updates, settings, etc.).
///
/// # Example
///
/// ```rust,no_run
/// use bulk_sdk::*;
///
/// let signer = TransactionSigner::generate();
/// let pk = signer.pubkey();
///
/// let order = LimitOrder::new("BTC-USD", Side::Buy, 98_000.0, 1.0);
/// let mut bundle = OrderBundle::new(vec![order.into()], signer.make_nonce(), pk, pk);
///
/// signer.sign(&mut bundle).unwrap();
/// let ws_json = bundle.to_ws_request(1).unwrap();
/// ```
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct TransactionSigner {
    signing_key: SigningKey,
    public_key: VerifyingKey,
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

        if key_bytes.len() < 32 {
            bail!("private key {} is wrong size", key_b58);
        }

        let seed: [u8; 32] = key_bytes[..32]
            .try_into()
            .map_err(|_| eyre::eyre!("failed to extract 32-byte seed"))?;

        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            public_key,
        })
    }

    /// Sign any [`Signable`] transaction in-place.
    ///
    /// Serializes the transaction, signs with Ed25519, and stores
    /// the raw 64-byte signature via [`Signable::set_signature`].
    pub fn sign<T: Signable>(&self, tx: &mut T) -> eyre::Result<()> {
        let message = tx.serialize()?;
        let signature = self.signing_key.sign(&message);
        tx.set_signature(signature.to_bytes());
        Ok(())
    }

    /// Sign an arbitrary byte slice and return the raw 64-byte signature.
    ///
    /// Used by [`BulkHttpClient`] for generic (non-`Signable`) payloads
    /// such as leverage updates, agent wallet management, and faucet requests,
    /// where the exchange expects a signature over the canonical JSON string.
    pub fn sign_bytes(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer as _;
        self.signing_key.sign(message).to_bytes()
    }

    /// Get pubkey
    pub fn public_key(&self) -> Pubkey {
        Pubkey::from(self.public_key.to_bytes())
    }

    /// Get pubkey as b58 encoding
    pub fn public_key_b58(&self) -> String {
        bs58::encode(self.public_key.to_bytes()).into_string()
    }
}
