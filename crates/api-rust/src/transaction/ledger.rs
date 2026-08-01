use crate::transaction::SignatureDomain;
use eyre::bail;
use hidapi::HidApi;
use solana_derivation_path::DerivationPath;
use solana_pubkey::Pubkey;
use solana_remote_wallet::{
    ledger::{is_valid_ledger, LedgerWallet},
    locator::Locator,
    remote_wallet::{RemoteWallet, RemoteWalletError},
};
use solana_signature::Signature;

const HID_GLOBAL_USAGE_PAGE: u16 = 0xFF00;
const HID_USB_DEVICE_CLASS: i32 = 0;
const OFFCHAIN_SIGNING_DOMAIN: &[u8; 16] = b"\xffsolana offchain";

/// Describes a connected Ledger device.
#[derive(Debug, Clone)]
pub struct LedgerDeviceInfo {
    pub model: String,
    pub serial: String,
    pub host_device_path: String,
    pub pubkey: Pubkey,
}

/// Describes the Ledger device and key selected by a locator.
#[derive(Debug, Clone)]
pub struct LedgerResolveInfo {
    pub locator: String,
    pub derivation_path: String,
    pub path: String,
    pub pubkey: Pubkey,
}

#[derive(Debug, Clone)]
pub(crate) struct LedgerSigner {
    locator: String,
    derivation_path: DerivationPath,
    confirm_key: bool,
    keypair_name: String,
    pubkey: Pubkey,
}

struct EnumeratedLedger {
    model: String,
    serial: String,
    host_device_path: String,
    base_pubkey: Pubkey,
}

struct ResolvedLedgerWallet {
    wallet: LedgerWallet,
    host_device_path: String,
    derived_pubkey: Pubkey,
}

impl LedgerSigner {
    // ───── Construction & Discovery ────────────────────────────────────────────────────────────

    /// Creates a Ledger signer and resolves its derived public key.
    ///
    /// # Arguments
    /// * `locator` - Remote-wallet locator used to select a Ledger device.
    /// * `derivation_path` - Optional Solana derivation path; defaults to `0/0`.
    /// * `confirm_key` - Whether the device must confirm the derived public key.
    /// * `keypair_name` - Display name associated with the Ledger keypair.
    ///
    /// # Returns
    /// A Ledger signer configured for the selected device and key.
    pub(crate) fn new(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<Self> {
        let derivation_path = Self::parse_derivation_path(derivation_path)?;
        let resolved = Self::resolve_wallet(locator, &derivation_path, confirm_key, keypair_name)?;
        Ok(Self {
            locator: locator.to_string(),
            derivation_path,
            confirm_key,
            keypair_name: keypair_name.to_string(),
            pubkey: resolved.derived_pubkey,
        })
    }

    /// Lists connected Ledger devices that can provide a Solana public key.
    ///
    /// # Returns
    /// Information about each available Ledger device.
    pub(crate) fn list_devices() -> eyre::Result<Vec<LedgerDeviceInfo>> {
        Ok(Self::enumerate_devices()?
            .into_iter()
            .map(|device| LedgerDeviceInfo {
                model: device.model,
                serial: device.serial,
                host_device_path: device.host_device_path,
                pubkey: device.base_pubkey,
            })
            .collect())
    }

    /// Resolves a Ledger locator and derivation path to a device and public key.
    ///
    /// # Arguments
    /// * `locator` - Remote-wallet locator used to select a Ledger device.
    /// * `derivation_path` - Optional Solana derivation path; defaults to `0/0`.
    /// * `confirm_key` - Whether the device must confirm the derived public key.
    /// * `keypair_name` - Display name associated with the Ledger keypair.
    ///
    /// # Returns
    /// The selected device path and derived Ledger public key.
    pub(crate) fn resolve_info(
        locator: &str,
        derivation_path: Option<&str>,
        confirm_key: bool,
        keypair_name: &str,
    ) -> eyre::Result<LedgerResolveInfo> {
        let derivation_path = Self::parse_derivation_path(derivation_path)?;
        let resolved = Self::resolve_wallet(locator, &derivation_path, confirm_key, keypair_name)?;
        Ok(LedgerResolveInfo {
            locator: locator.to_string(),
            derivation_path: format!("{derivation_path:?}"),
            path: resolved.host_device_path,
            pubkey: resolved.derived_pubkey,
        })
    }

    // ───── Accessors ───────────────────────────────────────────────────────────────────────────

    /// Returns the public key derived by this Ledger signer.
    pub(crate) fn public_key(&self) -> Pubkey {
        self.pubkey
    }

    // ───── Signing ─────────────────────────────────────────────────────────────────────────────

    /// Signs a generic authenticated payload using a Solana offchain envelope.
    ///
    /// # Arguments
    /// * `message` - Payload bytes to sign.
    /// * `signature_domain` - Bulk network domain bound to the signature.
    ///
    /// # Returns
    /// The Ledger-produced signature.
    pub(crate) fn sign_bytes(
        &self,
        message: &[u8],
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        self.sign_offchain(message, signature_domain)
    }

    /// Signs serialized transaction bytes using the Ledger transaction envelope.
    ///
    /// # Arguments
    /// * `message` - Serialized transaction bytes to sign.
    /// * `signature_domain` - Bulk network domain bound to the signature.
    ///
    /// # Returns
    /// The Ledger-produced transaction signature.
    pub(crate) fn sign_transaction_bytes(
        &self,
        message: &[u8],
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        let payload = format!("bulk-tx:{}", bs58::encode(message).into_string());
        self.sign_offchain(payload.as_bytes(), signature_domain)
    }

    /// Signs a human-readable transaction using a Solana offchain envelope.
    ///
    /// # Arguments
    /// * `clear_text` - Human-readable transaction content to sign.
    /// * `signature_domain` - Bulk network domain bound to the signature.
    ///
    /// # Returns
    /// The Ledger-produced transaction signature.
    pub(crate) fn sign_transaction_clear(
        &self,
        clear_text: &str,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        self.sign_offchain(clear_text.as_bytes(), signature_domain)
    }

    fn sign_offchain(
        &self,
        payload: &[u8],
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Signature> {
        let resolved = Self::resolve_wallet(
            &self.locator,
            &self.derivation_path,
            self.confirm_key,
            &self.keypair_name,
        )?;
        let envelope =
            Self::offchain_message_envelope_bytes(payload, &self.pubkey, signature_domain)?;
        resolved
            .wallet
            .sign_offchain_message(&self.derivation_path, &envelope)
            .map_err(|error| eyre::eyre!("ledger sign failed: {error}"))
    }

    // ───── Device Resolution ───────────────────────────────────────────────────────────────────

    fn resolve_wallet(
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
        let mut fallback_match = None;

        for info in hid.device_list() {
            let strict = is_valid_ledger(info.vendor_id(), info.product_id());
            if !Self::is_supported_device(info, strict, strict_seen) {
                continue;
            }
            if strict {
                strict_seen = true;
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

    fn enumerate_devices() -> eyre::Result<Vec<EnumeratedLedger>> {
        let mut hid = HidApi::new()?;
        hid.refresh_devices()?;
        let mut devices = Vec::new();
        let mut strict_seen = false;

        for info in hid.device_list() {
            let strict = is_valid_ledger(info.vendor_id(), info.product_id());
            if !Self::is_supported_device(info, strict, strict_seen) {
                continue;
            }
            if strict {
                strict_seen = true;
            }
            let Ok(device) = hid.open_path(info.path()) else {
                continue;
            };
            let mut wallet = LedgerWallet::new(device);
            let Ok(remote_info) = wallet.read_device(info) else {
                continue;
            };
            devices.push(EnumeratedLedger {
                model: remote_info.model,
                serial: remote_info.serial,
                host_device_path: remote_info.host_device_path,
                base_pubkey: remote_info.pubkey,
            });
        }

        Ok(devices)
    }

    fn is_supported_device(info: &hidapi::DeviceInfo, strict: bool, strict_seen: bool) -> bool {
        let fallback = info.vendor_id() == 0x2c97;
        let hid_ok = info.usage_page() == HID_GLOBAL_USAGE_PAGE
            || info.interface_number() == HID_USB_DEVICE_CLASS;
        (strict || fallback) && hid_ok && (!strict_seen || strict)
    }

    // ───── Message Encoding ────────────────────────────────────────────────────────────────────

    fn offchain_message_envelope_bytes(
        payload: &[u8],
        signer: &Pubkey,
        signature_domain: SignatureDomain,
    ) -> eyre::Result<Vec<u8>> {
        if payload.is_empty() {
            bail!("offchain payload cannot be empty");
        }
        if payload.len() > u16::MAX as usize {
            bail!("offchain payload too large");
        }
        let ascii = payload.iter().all(|byte| (0x20..=0x7e).contains(byte));
        let utf8 = std::str::from_utf8(payload).is_ok();
        let format = if ascii {
            0u8
        } else if utf8 {
            1u8
        } else {
            bail!("offchain payload must be ASCII or UTF-8");
        };

        let mut envelope = Vec::with_capacity(16 + 1 + 32 + 1 + 1 + 32 + 2 + payload.len());
        envelope.extend_from_slice(OFFCHAIN_SIGNING_DOMAIN);
        envelope.push(0);
        envelope.push(signature_domain as u8);
        envelope.extend_from_slice(&[0u8; 31]);
        envelope.push(format);
        envelope.push(1);
        envelope.extend_from_slice(signer.as_ref());
        envelope.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        envelope.extend_from_slice(payload);
        Ok(envelope)
    }

    fn parse_derivation_path(input: Option<&str>) -> eyre::Result<DerivationPath> {
        let Some(path) = input.map(str::trim).filter(|path| !path.is_empty()) else {
            return DerivationPath::from_key_str("0/0").map_err(|error| {
                eyre::eyre!("failed to set default derivation path 0/0: {error}")
            });
        };
        if path.starts_with("m/") {
            DerivationPath::from_absolute_path_str(path)
                .map_err(|error| eyre::eyre!("invalid absolute derivation path `{path}`: {error}"))
        } else {
            DerivationPath::from_key_str(path)
                .map_err(|error| eyre::eyre!("invalid derivation path `{path}`: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_offchain_envelope_uses_first_app_domain_byte() {
        let envelope = LedgerSigner::offchain_message_envelope_bytes(
            b"Bulk Exchange Transaction",
            &Pubkey::new_unique(),
            SignatureDomain::Testnet,
        )
        .expect("offchain envelope");

        assert_eq!(envelope[17], SignatureDomain::Testnet as u8);
        assert_eq!(&envelope[18..49], &[0; 31]);
    }
}
