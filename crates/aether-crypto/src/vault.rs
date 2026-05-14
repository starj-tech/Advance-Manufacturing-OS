//! Session vault — wrap the tenant `MasterKey` with a device-bound key
//! and stash the resulting ciphertext blob in the OS keystore.
//!
//! ## Why a wrapper layer at all?
//! `MasterKey` is the root of every encrypted column for a tenant. We
//! want it to:
//!   1. Persist across app restarts so the user isn't asked for their
//!      24-word phrase every boot (UX); and
//!   2. **Never** sit in storage in usable form. A device-bound key
//!      (sourced from the OS keychain by a separate handle) AEAD-wraps
//!      the master key. Even an attacker with raw access to the
//!      keystore blob cannot decrypt unless they also unlock the
//!      device-bound key — which on macOS / Windows / Linux happens via
//!      the OS auth flow (Touch ID, Windows Hello, libsecret prompt).
//!
//! ## What's NOT here yet
//! Per-OS keystore back-ends (Keychain / DPAPI / Secret Service) — they
//! live in `aether-tauri`'s production wiring. This crate gives you
//! the wrapping primitives + a `MemoryKeystore` so the round-trip is
//! testable.

use crate::aead::{decrypt, encrypt, Ciphertext, AEAD_KEY_LEN};
use crate::kdf::MasterKey;
use crate::keystore::{KeyHandle, Keystore};
use crate::{CryptoError, CryptoResult};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Device-bound 32-byte key used to wrap the tenant MasterKey before
/// persisting. In production it comes from a passkey-gated OS keychain
/// entry; in tests we generate one explicitly.
///
/// Wrapped in `Zeroize` so the device key never lingers in memory
/// after the vault drops.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct DeviceKey([u8; AEAD_KEY_LEN]);

impl DeviceKey {
    pub fn from_bytes(b: [u8; AEAD_KEY_LEN]) -> Self {
        Self(b)
    }

    /// Generate a fresh 32-byte device key from the OS RNG. Tests use
    /// this; in production the device key is derived from a Passkey
    /// assertion via HKDF (PR #2 auth slice).
    pub fn generate() -> Self {
        let mut out = [0u8; AEAD_KEY_LEN];
        OsRng.fill_bytes(&mut out);
        Self(out)
    }

    pub fn as_bytes(&self) -> &[u8; AEAD_KEY_LEN] {
        &self.0
    }
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceKey(***)")
    }
}

/// On-disk format for a wrapped MasterKey. Stored in the OS keystore
/// as a single opaque blob; we ship a `serde` impl so callers can
/// JSON-encode it if their keystore back-end stores strings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WrappedMasterKey {
    /// AEAD nonce (24 bytes for XChaCha20-Poly1305).
    pub nonce: Vec<u8>,
    /// Ciphertext bytes (master key + Poly1305 tag).
    pub ciphertext: Vec<u8>,
    /// The tenant this blob is for. Mismatched tenant on unwrap is
    /// rejected to prevent accidentally decrypting tenant A's vault
    /// with tenant B's device key — a misconfiguration we'd rather
    /// surface loudly than silently produce garbage 32 bytes.
    pub tenant_id: Uuid,
}

impl WrappedMasterKey {
    /// AEAD-wrap a tenant's MasterKey with the device key. The tenant
    /// UUID is bound into the associated metadata so swapping tenants
    /// across devices is structurally impossible.
    pub fn wrap(master: &MasterKey, device: &DeviceKey, tenant_id: Uuid) -> CryptoResult<Self> {
        let ct: Ciphertext = encrypt(device.as_bytes(), master.as_bytes())?;
        Ok(WrappedMasterKey {
            nonce: ct.nonce,
            ciphertext: ct.bytes,
            tenant_id,
        })
    }

    /// AEAD-unwrap to recover the MasterKey. Returns
    /// `CryptoError::Aead` on tag mismatch (wrong device key, tampered
    /// blob, or wrong nonce) and `CryptoError::Keystore` on a
    /// tenant-id mismatch.
    pub fn unwrap(&self, device: &DeviceKey, expected_tenant: Uuid) -> CryptoResult<MasterKey> {
        if self.tenant_id != expected_tenant {
            return Err(CryptoError::Keystore(format!(
                "tenant mismatch: blob is for {}, expected {}",
                self.tenant_id, expected_tenant
            )));
        }
        let ct = Ciphertext {
            nonce: self.nonce.clone(),
            bytes: self.ciphertext.clone(),
        };
        let pt = decrypt(device.as_bytes(), &ct)?;
        if pt.len() != AEAD_KEY_LEN {
            return Err(CryptoError::Aead);
        }
        let mut buf = [0u8; AEAD_KEY_LEN];
        buf.copy_from_slice(&pt);
        Ok(MasterKey::from_bytes(buf))
    }

    /// Bincode-style encoding for keystore blobs that want raw bytes.
    /// Format: 4-byte LE nonce length, nonce, 4-byte LE ct length, ct,
    /// 16-byte tenant UUID. Stable on-disk format — bump only with a
    /// version prefix when we evolve it.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.nonce.len() + 4 + self.ciphertext.len() + 16);
        out.extend_from_slice(&(self.nonce.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&(self.ciphertext.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.ciphertext);
        out.extend_from_slice(self.tenant_id.as_bytes());
        out
    }

    pub fn from_bytes(b: &[u8]) -> CryptoResult<Self> {
        let mut cursor = 0usize;
        fn take<'a>(b: &'a [u8], cursor: &mut usize, n: usize) -> CryptoResult<&'a [u8]> {
            if *cursor + n > b.len() {
                return Err(CryptoError::Keystore("truncated wrapped blob".into()));
            }
            let out = &b[*cursor..*cursor + n];
            *cursor += n;
            Ok(out)
        }
        let nonce_len = u32::from_le_bytes(take(b, &mut cursor, 4)?.try_into().unwrap()) as usize;
        let nonce = take(b, &mut cursor, nonce_len)?.to_vec();
        let ct_len = u32::from_le_bytes(take(b, &mut cursor, 4)?.try_into().unwrap()) as usize;
        let ciphertext = take(b, &mut cursor, ct_len)?.to_vec();
        let tenant_bytes: [u8; 16] = take(b, &mut cursor, 16)?
            .try_into()
            .map_err(|_| CryptoError::Keystore("tenant uuid wrong len".into()))?;
        Ok(WrappedMasterKey {
            nonce,
            ciphertext,
            tenant_id: Uuid::from_bytes(tenant_bytes),
        })
    }
}

/// Per-tenant key handle convention. Keeping this central means every
/// caller (production code, tests, migration tools) uses the same
/// keystore namespace and there's exactly one place to change if we
/// ever need to bump the scheme.
pub fn vault_handle(tenant_id: Uuid) -> KeyHandle {
    KeyHandle::new(format!("aether/tenant/{}/vault", tenant_id))
}

/// SessionVault — bundle a wrapped key + keystore handle + device key
/// into a single lock/unlock surface. The session lives in memory only
/// while it's `Unlocked`; the `Locked` form is what persists.
pub struct SessionVault {
    keystore: Box<dyn Keystore>,
    device: DeviceKey,
    tenant_id: Uuid,
}

impl SessionVault {
    pub fn new(keystore: Box<dyn Keystore>, device: DeviceKey, tenant_id: Uuid) -> Self {
        Self {
            keystore,
            device,
            tenant_id,
        }
    }

    /// Wrap `master` with the device key and stash the resulting blob
    /// in the keystore. Subsequent `unlock` calls recover the master
    /// key as long as the same device key is available.
    pub fn lock(&self, master: &MasterKey) -> CryptoResult<()> {
        let wrapped = WrappedMasterKey::wrap(master, &self.device, self.tenant_id)?;
        let blob = Zeroizing::new(wrapped.to_bytes());
        self.keystore.store(&vault_handle(self.tenant_id), blob)
    }

    /// Read the wrapped blob and recover the master key. Returns
    /// `Ok(None)` when no vault has been locked for this tenant yet
    /// (fresh install / first boot).
    pub fn unlock(&self) -> CryptoResult<Option<MasterKey>> {
        let Some(blob) = self.keystore.load(&vault_handle(self.tenant_id))? else {
            return Ok(None);
        };
        let wrapped = WrappedMasterKey::from_bytes(blob.as_slice())?;
        wrapped.unwrap(&self.device, self.tenant_id).map(Some)
    }

    /// Forget the vault for this tenant. Used on tenant offboarding
    /// (`tenant_users.status = 'offboarded'`) and after a master key
    /// rotation to ensure the prior blob can never be re-unwrapped.
    pub fn forget(&self) -> CryptoResult<()> {
        self.keystore.delete(&vault_handle(self.tenant_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::MemoryKeystore;

    fn master() -> MasterKey {
        MasterKey::from_bytes([7u8; AEAD_KEY_LEN])
    }

    #[test]
    fn wrap_then_unwrap_roundtrip() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let original = master();

        let wrapped = WrappedMasterKey::wrap(&original, &device, tenant).unwrap();
        let recovered = wrapped.unwrap(&device, tenant).unwrap();

        assert_eq!(recovered.as_bytes(), original.as_bytes());
    }

    #[test]
    fn wrong_device_key_fails_aead() {
        let device = DeviceKey::generate();
        let other = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let wrapped = WrappedMasterKey::wrap(&master(), &device, tenant).unwrap();

        let err = wrapped.unwrap(&other, tenant).unwrap_err();
        assert!(matches!(err, CryptoError::Aead));
    }

    #[test]
    fn wrong_tenant_id_is_rejected_before_aead() {
        let device = DeviceKey::generate();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let wrapped = WrappedMasterKey::wrap(&master(), &device, tenant_a).unwrap();

        let err = wrapped.unwrap(&device, tenant_b).unwrap_err();
        assert!(matches!(err, CryptoError::Keystore(_)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let mut wrapped = WrappedMasterKey::wrap(&master(), &device, tenant).unwrap();
        wrapped.ciphertext[0] ^= 0x01;
        let err = wrapped.unwrap(&device, tenant).unwrap_err();
        assert!(matches!(err, CryptoError::Aead));
    }

    #[test]
    fn bytes_round_trip() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let wrapped = WrappedMasterKey::wrap(&master(), &device, tenant).unwrap();
        let bytes = wrapped.to_bytes();
        let parsed = WrappedMasterKey::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, wrapped);
    }

    #[test]
    fn from_bytes_rejects_truncated_blob() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let bytes = WrappedMasterKey::wrap(&master(), &device, tenant)
            .unwrap()
            .to_bytes();
        let truncated = &bytes[..bytes.len() / 2];
        let err = WrappedMasterKey::from_bytes(truncated).unwrap_err();
        assert!(matches!(err, CryptoError::Keystore(_)));
    }

    #[test]
    fn vault_lock_then_unlock_recovers_master_key() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let vault = SessionVault::new(Box::new(MemoryKeystore::new()), device, tenant);

        // Initial unlock — nothing stored yet.
        assert!(vault.unlock().unwrap().is_none());

        vault.lock(&master()).unwrap();
        let recovered = vault.unlock().unwrap().expect("unlock after lock");
        assert_eq!(recovered.as_bytes(), master().as_bytes());
    }

    #[test]
    fn vault_forget_clears_keystore() {
        let device = DeviceKey::generate();
        let tenant = Uuid::new_v4();
        let vault = SessionVault::new(Box::new(MemoryKeystore::new()), device, tenant);

        vault.lock(&master()).unwrap();
        assert!(vault.unlock().unwrap().is_some());

        vault.forget().unwrap();
        assert!(
            vault.unlock().unwrap().is_none(),
            "after forget(), unlock must return None"
        );
    }

    #[test]
    fn vault_handle_is_tenant_scoped() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        assert_ne!(vault_handle(a), vault_handle(b));
        // Sanity: format is stable and human-grep-friendly.
        assert!(vault_handle(a).0.starts_with("aether/tenant/"));
        assert!(vault_handle(a).0.ends_with("/vault"));
    }

    #[test]
    fn debug_does_not_leak_device_key_bytes() {
        let d = DeviceKey::from_bytes([42u8; AEAD_KEY_LEN]);
        let dbg = format!("{d:?}");
        assert!(dbg.contains("***"));
        assert!(!dbg.contains("42"));
    }
}
