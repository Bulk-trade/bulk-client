use crate::transaction::SignatureDomain;
use eyre::{bail, WrapErr};
use hidapi::HidApi;
use solana_derivation_path::DerivationPath;
use solana_pubkey::Pubkey;
use solana_remote_wallet::{
    ledger::{is_valid_ledger, LedgerWallet},
    locator::Locator,
    remote_wallet::RemoteWallet,
};
use solana_signature::Signature;
use std::{thread, time::Duration};

const HID_GLOBAL_USAGE_PAGE: u16 = 0xFF00;
const HID_USB_DEVICE_CLASS: i32 = 0;
const LEDGER_DISCOVERY_ATTEMPTS: usize = 4;
const LEDGER_DISCOVERY_RETRY_DELAY: Duration = Duration::from_millis(250);
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
        let target_pubkey = Locator::new_from_path(locator)?.pubkey;
        let mut last_error = None;

        for attempt in 1..=LEDGER_DISCOVERY_ATTEMPTS {
            match Self::resolve_wallet_once(target_pubkey, derivation_path, confirm_key) {
                Ok(wallet) => return Ok(wallet),
                Err(error) => last_error = Some(error),
            }
            if attempt < LEDGER_DISCOVERY_ATTEMPTS {
                thread::sleep(LEDGER_DISCOVERY_RETRY_DELAY);
            }
        }

        let last_error = last_error.expect("Ledger discovery always makes at least one attempt");
        let context = Self::ledger_discovery_error_context(&format!("{last_error:#}"));
        Err(last_error).wrap_err(context)
    }

    fn resolve_wallet_once(
        target_pubkey: Option<Pubkey>,
        derivation_path: &DerivationPath,
        confirm_key: bool,
    ) -> eyre::Result<ResolvedLedgerWallet> {
        let mut hid = HidApi::new()?;
        hid.refresh_devices()?;
        let mut strict_seen = false;
        let mut fallback_match = None;
        let mut candidates = 0;
        let mut failures = Vec::new();

        for info in hid.device_list() {
            let strict = is_valid_ledger(info.vendor_id(), info.product_id());
            if !Self::is_supported_device(info, strict, strict_seen) {
                continue;
            }
            if strict {
                strict_seen = true;
            }
            candidates += 1;
            let device = match hid.open_path(info.path()) {
                Ok(device) => device,
                Err(error) => {
                    failures.push(format!(
                        "could not open {}: {error}",
                        info.path().to_string_lossy()
                    ));
                    continue;
                }
            };
            let mut wallet = LedgerWallet::new(device);
            let remote_info = match wallet.read_device(info) {
                Ok(remote_info) => remote_info,
                Err(error) => {
                    failures.push(format!(
                        "could not read {}: {error}",
                        info.path().to_string_lossy()
                    ));
                    continue;
                }
            };
            let derived_pubkey = match wallet.get_pubkey(derivation_path, confirm_key) {
                Ok(pubkey) => pubkey,
                Err(error) => {
                    failures.push(format!(
                        "could not derive a public key from {}: {error}",
                        remote_info.host_device_path
                    ));
                    continue;
                }
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

        if let Some(wallet) = fallback_match {
            return Ok(wallet);
        }
        if candidates == 0 {
            bail!("no supported Ledger HID interfaces were found");
        }
        if failures.is_empty() {
            bail!("no connected Ledger matched the requested public key");
        }
        bail!(
            "found {candidates} Ledger HID interface(s), but none became usable: {}",
            failures.join("; ")
        )
    }

    fn ledger_discovery_error_context(error: &str) -> String {
        if error.contains("not permitted") || error.contains("0xE00002E2") {
            return "Ledger detected, but macOS denied access to its USB HID interface. Close Ledger Live and other wallet apps, reconnect and unlock the device, open the Solana app, and check the terminal application's Privacy & Security permissions".to_string();
        }
        if error.contains("could not open") {
            return "Ledger detected, but its USB HID interface could not be opened. Close Ledger Live and other wallet apps, then reconnect and unlock the device"
                .to_string();
        }
        if error.contains("could not read") || error.contains("could not derive a public key") {
            return "Ledger detected, but the Solana app did not respond. Unlock the device, open the Solana app, and ensure blind signing is enabled if required"
                .to_string();
        }
        if error.contains("matched the requested public key") {
            return "Ledger connected, but it does not match the requested signer. Verify the device and derivation path"
                .to_string();
        }
        format!(
            "Ledger was not available after {LEDGER_DISCOVERY_ATTEMPTS} attempts. Reconnect and unlock the device, then open the Solana app"
        )
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

    #[test]
    fn ledger_discovery_error_identifies_macos_permission_denial() {
        let context = LedgerSigner::ledger_discovery_error_context(
            "hid_open_path failed: (0xE00002E2) not permitted",
        );

        assert!(context.starts_with("Ledger detected, but macOS denied access"));
    }

    #[test]
    fn ledger_discovery_error_identifies_unresponsive_solana_app() {
        let context = LedgerSigner::ledger_discovery_error_context(
            "found device, but could not derive a public key",
        );

        assert!(context.starts_with("Ledger detected, but the Solana app did not respond"));
    }
}
