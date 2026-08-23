#[cfg(feature = "ledger")]
pub use crate::transaction::ledger::{LedgerDeviceInfo, LedgerResolveInfo};
#[cfg(feature = "ledger")]
use crate::transaction::ledger::LedgerSigner;
use crate::transaction::SignatureDomain;
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
/// tx.sign(&signer, SignatureDomain::Devnet)?;
/// ```
#[derive(Debug)]
#[allow(unused)]
pub struct TransactionSigner {
    kind: SignerKind,
}

#[derive(Debug)]
enum SignerKind {
    Software(Keypair),
    #[cfg(feature = "ledger")]
    Ledger(LedgerSigner),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxSignatureMode {
    Raw,
    Offchain,
}

#[allow(unused)]
impl TransactionSigner {
    // ───── Public API Contract ─────────────────────────────────────────────────────────────────

    /// Creates a signer from a base58-encoded private key.
    ///
    /// # Arguments
    /// * `key_b58` - Base58-encoded 32-byte seed or 64-byte keypair.
    ///
    /// # Returns
    /// A software-backed transaction signer.
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
                "private key has an invalid length (got {} bytes; expected a 32-byte seed or 64-byte keypair)",
                key_bytes.len()
            );
        };

        Ok(Self {
            kind: SignerKind::Software(keypair),
        })
    }

    #[cfg(feature = "ledger")]
    /// Creates a signer for the Ledger device matching a locator.
    ///
    /// # Arguments
    /// * `locator` - Remote-wallet locator used to select a Ledger device.
    /// * `derivation_path` - Optional Solana derivation path; defaults to `0/0`.
    ///
    /// # Returns
    /// A Ledger-backed transaction signer.
    pub fn from_ledger(locator: &str, derivation_path: Option<&str>) -> eyre::Result<Self> {
        Self::from_ledger_with_options(locator, derivation_path, false, "bulk-cli")
    }

    #[cfg(feature = "ledger")]
    /// Creates a Ledger signer with explicit device confirmation options.
    ///
    /// # Arguments
    /// * `locator` - Remote-wallet locator used to select a Ledger device.
    /// * `derivation_path` - Optional Solana derivation path; defaults to `0/0`.
    /// * `confirm_key` - Whether the device must confirm the derived public key.
    /// * `keypair_name` - Display name associated with the Ledger keypair.
    ///
    /// # Returns
    /// A configured Ledger-backed transaction signer.
    pub fn from_ledger_with_options(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<Self> {
        Ok(Self {
            kind: SignerKind::Ledger(LedgerSigner::new(
                locator,
                derivation_path,
                confirm_key,
                keypair_name,
            )?),
        })
    }

    #[cfg(feature = "ledger")]
    /// Lists connected Ledger devices that can provide a Solana public key.
    ///
    /// # Returns
    /// Information about each available Ledger device.
    pub fn list_ledger_devices() -> eyre::Result<Vec<LedgerDeviceInfo>> {
        LedgerSigner::list_devices()
    }

    #[cfg(feature = "ledger")]
    /// Resolves a Ledger locator and derivation path to a concrete device and public key.
    ///
    /// # Arguments
    /// * `locator` - Remote-wallet locator used to select a Ledger device.
    /// * `derivation_path` - Optional Solana derivation path; defaults to `0/0`.
    /// * `confirm_key` - Whether the device must confirm the derived public key.
    /// * `keypair_name` - Display name associated with the Ledger keypair.
    ///
    /// # Returns
    /// The selected device path and derived Ledger public key.
    pub fn resolve_ledger_with_options(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<LedgerResolveInfo> {
        LedgerSigner::resolve_info(locator, derivation_path, confirm_key, keypair_name)
    }

    // ───── Accessors ───────────────────────────────────────────────────────────────────────────

    /// Returns the signing mode required for transaction signatures.
    pub fn tx_signature_mode(&self) -> TxSignatureMode {
        match &self.kind {
            SignerKind::Software(_) => TxSignatureMode::Raw,
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(_) => TxSignatureMode::Offchain,
        }
    }

    /// Returns the HTTP header value that identifies the transaction signature mode.
    pub fn tx_signature_mode_hint_header_value(&self) -> Option<&'static str> {
        match self.tx_signature_mode() {
            TxSignatureMode::Raw => None,
            TxSignatureMode::Offchain => Some("offchain"),
        }
    }

    /// Returns the signer's public key.
    pub fn public_key(&self) -> Pubkey {
        match &self.kind {
            SignerKind::Software(keypair) => keypair.pubkey(),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(ledger) => ledger.public_key(),
        }
    }

    /// Returns the signer's public key encoded as base58.
    pub fn public_key_b58(&self) -> String {
        self.public_key().to_string()
    }

    // ───── Signing ─────────────────────────────────────────────────────────────────────────────

    /// Sign a generic authenticated payload bound to one Bulk network.
    ///
    /// Used by [`BulkHttpClient`] for generic (non-`Signable`) payloads
    /// such as leverage updates, agent wallet management, and faucet requests,
    /// where software wallets sign `canonical_json || signature_domain_u8`.
    /// Ledger wallets bind the same domain in the first application-domain byte
    /// of the Solana offchain-message envelope.
    pub fn sign_bytes(
        &self,
        message: &[u8],
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => {
                let mut preimage = Vec::with_capacity(message.len() + 1);
                preimage.extend_from_slice(message);
                preimage.push(signature_domain as u8);
                Ok(keypair.sign_message(&preimage))
            }
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(ledger) => ledger.sign_bytes(message, signature_domain),
        }
    }

    /// Signs serialized transaction bytes in the format required by the signer type.
    ///
    /// # Arguments
    /// * `message` - Serialized transaction bytes to sign.
    /// * `signature_domain` - Bulk network domain bound to Ledger signatures.
    ///
    /// # Returns
    /// The transaction signature.
    pub fn sign_transaction_bytes(
        &self,
        message: &[u8],
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => Ok(keypair.sign_message(message)),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(ledger) => ledger.sign_transaction_bytes(message, signature_domain),
        }
    }

    /// Signs a human-readable transaction message.
    ///
    /// # Arguments
    /// * `clear_text` - Human-readable transaction content to sign.
    /// * `signature_domain` - Bulk network domain bound to Ledger signatures.
    ///
    /// # Returns
    /// The transaction signature.
    pub fn sign_transaction_clear(
        &self,
        clear_text: &str,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => Ok(keypair.sign_message(clear_text.as_bytes())),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(ledger) => {
                ledger.sign_transaction_clear(clear_text, signature_domain)
            }
        }
    }
}

impl Clone for TransactionSigner {
    fn clone(&self) -> Self {
        match &self.kind {
            SignerKind::Software(keypair) => Self {
                kind: SignerKind::Software(keypair.insecure_clone()),
            },
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(cfg) => Self {
                kind: SignerKind::Ledger(cfg.clone()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_private_key_errors_do_not_echo_secret_material() {
        let secret = "not-a-valid-private-key";
        let error = match TransactionSigner::from_private_key(secret) {
            Ok(_) => panic!("the key is invalid"),
            Err(error) => error,
        };
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn generic_signature_is_bound_to_one_domain() {

        let signer =
            TransactionSigner::from_private_key("1111111111111111111111111111111111111111111")
                .expect("test signer");
        let mainnet = signer
            .sign_bytes(b"{\"action\":\"faucet\"}", SignatureDomain::Mainnet)
            .expect("mainnet signature");
        let testnet = signer
            .sign_bytes(b"{\"action\":\"faucet\"}", SignatureDomain::Testnet)
            .expect("testnet signature");
        let mut mainnet_preimage = b"{\"action\":\"faucet\"}".to_vec();
        mainnet_preimage.push(SignatureDomain::Mainnet as u8);

        assert_ne!(mainnet, testnet);
        assert!(mainnet.verify(signer.public_key().as_ref(), &mainnet_preimage));
    }
}
