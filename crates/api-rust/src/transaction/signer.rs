use eyre::bail;
use solana_keypair::{keypair_from_seed, Keypair};
use solana_pubkey::Pubkey;

use solana_signature::Signature;
use solana_signer::Signer;

/// Ed25519 signer for Bulk exchange transactions.
///
/// # Example
///
/// ```text
/// let signer = TransactionSigner::from_private_key("base58_key")?;
/// let mut tx = Transaction { .. };
/// tx.sign(&signer)?;
/// ```
#[derive(Debug)]
#[allow(unused)]
pub struct TransactionSigner {
    keypair: Keypair,
}

#[allow(unused)]
impl TransactionSigner {
    /// Create a signer from a base58-encoded private key (32-byte seed).
    ///
    /// # Arguments
    /// - `private_key`: 32 or 64 byte private key
    pub fn from_private_key(key_b58: &str) -> eyre::Result<Self> {
        let key_bytes = bs58::decode(key_b58).into_vec()?;

        let keypair = if key_bytes.len() == 64 {
            // Full 64-byte keypair (secret + public)
            Keypair::try_from(key_bytes.as_slice())
                .map_err(|e| eyre::eyre!("invalid 64-byte keypair: {e}"))?
        } else if key_bytes.len() >= 32 {
            // 32-byte seed — derive the keypair
            keypair_from_seed(&key_bytes[..32])
                .map_err(|e| eyre::eyre!("failed to create keypair from seed: {e}"))?
        } else {
            bail!(
                "private key {} is wrong size (got {} bytes)",
                key_b58,
                key_bytes.len()
            );
        };

        Ok(Self { keypair })
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
