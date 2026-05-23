use eyre::bail;
use solana_keypair::{keypair_from_seed, Keypair};
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::Signer;
#[cfg(feature = "ledger")]
use {
    hidapi::HidApi,
    solana_derivation_path::DerivationPath,
    solana_remote_wallet::{
        ledger::{is_valid_ledger, LedgerWallet},
        locator::Locator,
        remote_wallet::{RemoteWallet, RemoteWalletError},
    },
};

#[cfg(feature = "ledger")]
const HID_GLOBAL_USAGE_PAGE: u16 = 0xFF00;
#[cfg(feature = "ledger")]
const HID_USB_DEVICE_CLASS: i32 = 0;
#[cfg(feature = "ledger")]
const OFFCHAIN_SIGNING_DOMAIN: &[u8; 16] = b"\xffsolana offchain";

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
    kind: SignerKind,
}

#[derive(Debug)]
enum SignerKind {
    Software(Keypair),
    #[cfg(feature = "ledger")]
    Ledger(LedgerConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxSignatureMode {
    Raw,
    Offchain,
}

#[cfg(feature = "ledger")]
#[derive(Debug, Clone)]
struct LedgerConfig {
    locator: String,
    derivation_path: DerivationPath,
    confirm_key: bool,
    keypair_name: String,
    pubkey: Pubkey,
}

#[cfg(feature = "ledger")]
#[derive(Debug, Clone)]
pub struct LedgerDeviceInfo {
    pub model: String,
    pub serial: String,
    pub host_device_path: String,
    pub pubkey: Pubkey,
}

#[cfg(feature = "ledger")]
#[derive(Debug, Clone)]
pub struct LedgerResolveInfo {
    pub locator: String,
    pub derivation_path: String,
    pub path: String,
    pub pubkey: Pubkey,
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

        Ok(Self {
            kind: SignerKind::Software(keypair),
        })
    }

    #[cfg(feature = "ledger")]
    pub fn from_ledger(locator: &str, derivation_path: Option<&str>) -> eyre::Result<Self> {
        Self::from_ledger_with_options(locator, derivation_path, false, "bulk-cli")
    }

    #[cfg(feature = "ledger")]
    pub fn from_ledger_with_options(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<Self> {
        let derivation_path = parse_derivation_path(derivation_path)?;
        let resolved = resolve_ledger_wallet(
            locator,
            &derivation_path,
            confirm_key,
            keypair_name,
        )?;
        Ok(Self {
            kind: SignerKind::Ledger(LedgerConfig {
                locator: locator.to_string(),
                derivation_path,
                confirm_key,
                keypair_name: keypair_name.to_string(),
                pubkey: resolved.derived_pubkey,
            }),
        })
    }

    #[cfg(feature = "ledger")]
    pub fn list_ledger_devices() -> eyre::Result<Vec<LedgerDeviceInfo>> {
        Ok(enumerate_ledger_devices()?
            .into_iter()
            .map(|d| LedgerDeviceInfo {
                model: d.model,
                serial: d.serial,
                host_device_path: d.host_device_path,
                pubkey: d.base_pubkey,
            })
            .collect())
    }

    #[cfg(feature = "ledger")]
    pub fn resolve_ledger_with_options(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<LedgerResolveInfo> {
        let derivation_path = parse_derivation_path(derivation_path)?;
        let resolved = resolve_ledger_wallet(
            locator,
            &derivation_path,
            confirm_key,
            keypair_name,
        )?;
        Ok(LedgerResolveInfo {
            locator: locator.to_string(),
            derivation_path: format!("{derivation_path:?}"),
            path: resolved.host_device_path,
            pubkey: resolved.derived_pubkey,
        })
    }

    /// Sign an arbitrary byte slice and return the raw 64-byte signature.
    ///
    /// Used by [`BulkHttpClient`] for generic (non-`Signable`) payloads
    /// such as leverage updates, agent wallet management, and faucet requests,
    /// where the exchange expects a signature over the canonical JSON string.
    pub fn sign_bytes(&self, message: &[u8]) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => Ok(keypair.sign_message(message)),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(cfg) => {
                let resolved = resolve_ledger_wallet(
                    &cfg.locator,
                    &cfg.derivation_path,
                    cfg.confirm_key,
                    &cfg.keypair_name,
                )?;
                let offchain = offchain_message_envelope_bytes(message, &cfg.pubkey)?;
                sign_ledger_offchain(&resolved.wallet, &cfg.derivation_path, message, &offchain)
            }
        }
    }

    pub fn sign_transaction_bytes(&self, message: &[u8]) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => Ok(keypair.sign_message(message)),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(cfg) => {
                let resolved = resolve_ledger_wallet(
                    &cfg.locator,
                    &cfg.derivation_path,
                    cfg.confirm_key,
                    &cfg.keypair_name,
                )?;
                let payload = format!("bulk-tx:{}", bs58::encode(message).into_string());
                let offchain = offchain_message_envelope_bytes(payload.as_bytes(), &cfg.pubkey)?;
                sign_ledger_offchain_strict(
                    &resolved.wallet,
                    &cfg.derivation_path,
                    &offchain,
                )
            }
        }
    }

    pub fn sign_transaction_clear(
        &self,
        clear_text: &str,
    ) -> eyre::Result<Signature> {
        match &self.kind {
            SignerKind::Software(keypair) => Ok(keypair.sign_message(clear_text.as_bytes())),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(cfg) => {
                let resolved = resolve_ledger_wallet(
                    &cfg.locator,
                    &cfg.derivation_path,
                    cfg.confirm_key,
                    &cfg.keypair_name,
                )?;
                let offchain =
                    offchain_message_envelope_bytes(clear_text.as_bytes(), &cfg.pubkey)?;
                sign_ledger_offchain_strict(
                    &resolved.wallet,
                    &cfg.derivation_path,
                    &offchain,
                )
            }
        }
    }

    pub fn tx_signature_mode(&self) -> TxSignatureMode {
        match &self.kind {
            SignerKind::Software(_) => TxSignatureMode::Raw,
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(_) => TxSignatureMode::Offchain,
        }
    }

    pub fn tx_signature_mode_hint_header_value(&self) -> Option<&'static str> {
        match self.tx_signature_mode() {
            TxSignatureMode::Raw => None,
            TxSignatureMode::Offchain => Some("offchain"),
        }
    }

    /// Get pubkey
    pub fn public_key(&self) -> Pubkey {
        match &self.kind {
            SignerKind::Software(keypair) => keypair.pubkey(),
            #[cfg(feature = "ledger")]
            SignerKind::Ledger(cfg) => cfg.pubkey,
        }
    }

    /// Get pubkey as b58 encoding
    pub fn public_key_b58(&self) -> String {
        self.public_key().to_string()
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

#[cfg(feature = "ledger")]
fn parse_derivation_path(input: Option<&str>) -> eyre::Result<DerivationPath> {
    let Some(path) = input.map(str::trim).filter(|s| !s.is_empty()) else {
        return DerivationPath::from_key_str("0/0")
            .map_err(|e| eyre::eyre!("failed to set default derivation path 0/0: {e}"));
    };
    if path.starts_with("m/") {
        DerivationPath::from_absolute_path_str(path)
            .map_err(|e| eyre::eyre!("invalid absolute derivation path `{path}`: {e}"))
    } else {
        DerivationPath::from_key_str(path)
            .map_err(|e| eyre::eyre!("invalid derivation path `{path}`: {e}"))
    }
}

#[cfg(feature = "ledger")]
struct EnumeratedLedger {
    model: String,
    serial: String,
    host_device_path: String,
    base_pubkey: Pubkey,
}

#[cfg(feature = "ledger")]
struct ResolvedLedgerWallet {
    wallet: LedgerWallet,
    host_device_path: String,
    derived_pubkey: Pubkey,
}

#[cfg(feature = "ledger")]
fn enumerate_ledger_devices() -> eyre::Result<Vec<EnumeratedLedger>> {
    let mut hid = HidApi::new()?;
    hid.refresh_devices()?;

    let mut infos = Vec::new();
    let mut strict_seen = false;

    for info in hid.device_list() {
        let strict = is_valid_ledger(info.vendor_id(), info.product_id());
        let fallback = info.vendor_id() == 0x2c97;
        let hid_ok =
            info.usage_page() == HID_GLOBAL_USAGE_PAGE || info.interface_number() == HID_USB_DEVICE_CLASS;
        if !strict && !fallback {
            continue;
        }
        if !hid_ok {
            continue;
        }
        if strict {
            strict_seen = true;
        }
        if strict_seen && !strict {
            continue;
        }

        let Ok(device) = hid.open_path(info.path()) else {
            continue;
        };
        let mut wallet = LedgerWallet::new(device);
        let Ok(remote_info) = wallet.read_device(info) else {
            continue;
        };
        infos.push(EnumeratedLedger {
            model: remote_info.model,
            serial: remote_info.serial,
            host_device_path: remote_info.host_device_path,
            base_pubkey: remote_info.pubkey,
        });
    }

    Ok(infos)
}

#[cfg(feature = "ledger")]
fn resolve_ledger_wallet(
    locator: &str,
    derivation_path: &DerivationPath,
    confirm_key: bool,
    _keypair_name: &str,
) -> eyre::Result<ResolvedLedgerWallet> {
    let locator = Locator::new_from_path(locator)?;
    let target_pubkey = locator.pubkey;

    let mut hid = HidApi::new()?;
    hid.refresh_devices()?;
    let mut strict_seen = false;

    let mut fallback_match: Option<ResolvedLedgerWallet> = None;
    for info in hid.device_list() {
        let strict = is_valid_ledger(info.vendor_id(), info.product_id());
        let fallback = info.vendor_id() == 0x2c97;
        let hid_ok =
            info.usage_page() == HID_GLOBAL_USAGE_PAGE || info.interface_number() == HID_USB_DEVICE_CLASS;
        if !strict && !fallback {
            continue;
        }
        if !hid_ok {
            continue;
        }
        if strict {
            strict_seen = true;
        }
        if strict_seen && !strict {
            continue;
        }

        let Ok(device) = hid.open_path(info.path()) else {
            continue;
        };
        let mut wallet = LedgerWallet::new(device);
        let Ok(remote_info) = wallet.read_device(info) else {
            continue;
        };
        let Ok(derived_pubkey) = wallet.get_pubkey(derivation_path, confirm_key) else {
            continue;
        };

        let candidate = ResolvedLedgerWallet {
            wallet,
            host_device_path: remote_info.host_device_path,
            derived_pubkey,
        };

        if let Some(target) = target_pubkey {
            if derived_pubkey == target || remote_info.pubkey == target {
                return Ok(candidate);
            }
            continue;
        }
        if fallback_match.is_none() {
            fallback_match = Some(candidate);
        }
    }

    fallback_match.ok_or_else(|| eyre::eyre!(RemoteWalletError::NoDeviceFound))
}

#[cfg(feature = "ledger")]
fn offchain_message_envelope_bytes(payload: &[u8], signer: &Pubkey) -> eyre::Result<Vec<u8>> {
    if payload.is_empty() {
        bail!("offchain payload cannot be empty");
    }
    if payload.len() > u16::MAX as usize {
        bail!("offchain payload too large");
    }
    let ascii = payload.iter().all(|b| (0x20..=0x7e).contains(b));
    let utf8 = std::str::from_utf8(payload).is_ok();
    let format = if ascii {
        0u8
    } else if utf8 {
        1u8
    } else {
        bail!("offchain payload must be ASCII or UTF-8");
    };

    let mut out = Vec::with_capacity(16 + 1 + 32 + 1 + 1 + 32 + 2 + payload.len());
    out.extend_from_slice(OFFCHAIN_SIGNING_DOMAIN);
    out.push(0);
    out.extend_from_slice(&[0u8; 32]);
    out.push(format);
    out.push(1);
    out.extend_from_slice(signer.as_ref());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

#[cfg(feature = "ledger")]
fn offchain_message_v0_bytes(payload: &[u8]) -> eyre::Result<Vec<u8>> {
    if payload.is_empty() {
        bail!("offchain payload cannot be empty");
    }
    if payload.len() > u16::MAX as usize {
        bail!("offchain payload too large");
    }
    let mut out = Vec::with_capacity(3 + payload.len());
    out.push(0); // restricted ascii
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

#[cfg(feature = "ledger")]
fn sign_ledger_offchain(
    wallet: &LedgerWallet,
    derivation_path: &DerivationPath,
    payload: &[u8],
    envelope: &[u8],
) -> eyre::Result<Signature> {
    match wallet.sign_offchain_message(derivation_path, envelope) {
        Ok(sig) => Ok(sig),
        Err(first_err) => {
            let msg = first_err.to_string().to_lowercase();
            if !msg.contains("invalid header") {
                return Err(eyre::eyre!("ledger sign failed: {first_err}"));
            }
            let v0 = offchain_message_v0_bytes(payload)?;
            match wallet.sign_offchain_message(derivation_path, &v0) {
                Ok(sig) => Ok(sig),
                Err(second_err) => {
                    let msg2 = second_err.to_string().to_lowercase();
                    if !msg2.contains("invalid header") {
                        return Err(eyre::eyre!("ledger sign failed: {second_err}"));
                    }
                    wallet
                        .sign_offchain_message(derivation_path, payload)
                        .map_err(|e| eyre::eyre!("ledger sign failed: {e}"))
                }
            }
        }
    }
}

#[cfg(feature = "ledger")]
fn sign_ledger_offchain_strict(
    wallet: &LedgerWallet,
    derivation_path: &DerivationPath,
    envelope: &[u8],
) -> eyre::Result<Signature> {
    wallet
        .sign_offchain_message(derivation_path, envelope)
        .map_err(|e| eyre::eyre!("ledger sign failed: {e}"))
}

