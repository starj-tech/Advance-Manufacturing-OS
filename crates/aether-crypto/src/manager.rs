//! `MasterKeyManager` — bootstrap orchestration on top of
//! `RecoveryPhrase` + `derive_master_key_for_tenant` +
//! `Keystore`.
//!
//! ## Bootstrap states
//! Every tenant goes through exactly three states across the
//! lifetime of a device:
//!
//!   1. **Unbound** — no key in the keystore for this tenant.
//!      First-boot flow calls [`MasterKeyManager::bootstrap`]
//!      which generates a fresh recovery phrase, derives the
//!      master key, and persists it. The caller MUST show the
//!      operator the returned phrase exactly once and confirm
//!      it was recorded (paper, password manager, etc.).
//!
//!   2. **Bound** — key in keystore. Subsequent boots call
//!      [`MasterKeyManager::unlock`] which returns the key.
//!      No phrase needed.
//!
//!   3. **Restoring** — operator has the phrase but the device
//!      is fresh (lost device, reinstall, new shop floor
//!      tablet). [`MasterKeyManager::restore_from_phrase`]
//!      re-derives the same key from the phrase + tenant UUID
//!      and persists it under the keystore handle, returning
//!      the device to Bound state.
//!
//! ## Why tenant UUID is the salt
//! `derive_master_key_for_tenant` already takes a tenant Uuid
//! as the Argon2id salt. The same phrase derives DIFFERENT
//! master keys per tenant, which means a service provider can
//! hold an emergency-envelope copy of every tenant's phrase
//! without any tenant's master being derivable from any other
//! tenant's data.
//!
//! ## Keystore handle convention
//! `tenant-<uuid>/master` — namespaced so multi-tenant
//! desktop installs don't collide. The OS keychain enforces
//! per-process isolation.

use crate::kdf::MasterKey;
use crate::keystore::{KeyHandle, Keystore};
use crate::recovery::{derive_master_key_for_tenant, RecoveryPhrase};
use crate::{CryptoError, CryptoResult};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Construct the keystore handle for a tenant's master key.
/// Centralized so the namespacing convention is consistent
/// across bootstrap / unlock / restore paths.
pub fn master_handle(tenant_id: Uuid) -> KeyHandle {
    KeyHandle::new(format!("tenant-{tenant_id}/master"))
}

/// Output of [`MasterKeyManager::bootstrap`]. The recovery
/// phrase must be surfaced to the operator EXACTLY ONCE; the
/// manager itself never persists the phrase, only the derived
/// master key (in the keystore).
#[derive(Debug)]
pub struct BootstrapResult {
    pub master_key: MasterKey,
    pub recovery_phrase: RecoveryPhrase,
}

pub struct MasterKeyManager<K: Keystore> {
    keystore: K,
}

impl<K: Keystore> MasterKeyManager<K> {
    pub fn new(keystore: K) -> Self {
        Self { keystore }
    }

    /// Borrow the underlying keystore. Useful for tests that
    /// need to verify keystore state directly.
    pub fn keystore(&self) -> &K {
        &self.keystore
    }

    /// First-time bootstrap. Generates a fresh 24-word
    /// recovery phrase, derives the master key against the
    /// tenant UUID as salt, persists the master key under the
    /// keystore handle, and returns the phrase + key.
    ///
    /// Refuses to overwrite an existing key — call sites that
    /// genuinely want to re-bootstrap MUST first call
    /// `keystore.delete(master_handle(tenant_id))` and own the
    /// consequences (existing encrypted data is now
    /// unrecoverable).
    pub fn bootstrap(&self, tenant_id: Uuid) -> CryptoResult<BootstrapResult> {
        let handle = master_handle(tenant_id);
        if self.keystore.load(&handle)?.is_some() {
            return Err(CryptoError::InvalidParameter(
                "tenant already bootstrapped — call delete first if intentional",
            ));
        }
        let phrase = RecoveryPhrase::generate()?;
        let master = derive_master_key_for_tenant(&phrase, tenant_id)?;
        self.keystore
            .store(&handle, Zeroizing::new(master.as_bytes().to_vec()))?;
        Ok(BootstrapResult {
            master_key: master,
            recovery_phrase: phrase,
        })
    }

    /// Subsequent-boot unlock. Loads the master key from the
    /// keystore under the tenant handle. Returns
    /// `CryptoError::InvalidParameter` if no key is bound (the
    /// caller should route to the Restoring flow:
    /// `restore_from_phrase`).
    pub fn unlock(&self, tenant_id: Uuid) -> CryptoResult<MasterKey> {
        let handle = master_handle(tenant_id);
        let bytes = self
            .keystore
            .load(&handle)?
            .ok_or(CryptoError::InvalidParameter(
                "tenant not bootstrapped on this device",
            ))?;
        if bytes.len() != 32 {
            return Err(CryptoError::InvalidParameter(
                "stored master key has wrong length",
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(MasterKey::from_bytes(arr))
    }

    /// Restore from a recovery phrase on a fresh device. Re-
    /// derives the same master key the original bootstrap
    /// produced (Argon2id with the tenant UUID as salt is
    /// deterministic). Persists the key under the keystore
    /// handle, returning the device to Bound state.
    ///
    /// REPLACES any existing key under that handle. The flow
    /// is "operator typed the phrase, so they own this device
    /// now" — if the key was wrong (mistyped phrase), no data
    /// will decrypt, which is the right failure mode (vs.
    /// silently overwriting and locking out the correct key).
    pub fn restore_from_phrase(
        &self,
        phrase: &RecoveryPhrase,
        tenant_id: Uuid,
    ) -> CryptoResult<MasterKey> {
        let master = derive_master_key_for_tenant(phrase, tenant_id)?;
        let handle = master_handle(tenant_id);
        self.keystore
            .store(&handle, Zeroizing::new(master.as_bytes().to_vec()))?;
        Ok(master)
    }

    /// Whether the tenant has been bootstrapped on this
    /// device. Cheap (single keystore load). Caller branches
    /// between unlock vs bootstrap on first run.
    pub fn is_bootstrapped(&self, tenant_id: Uuid) -> CryptoResult<bool> {
        Ok(self.keystore.load(&master_handle(tenant_id))?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::MemoryKeystore;

    fn tenant() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn master_handle_namespaces_per_tenant() {
        let a = tenant();
        let b = tenant();
        assert_ne!(master_handle(a), master_handle(b));
        // Format anchor — load-bearing for multi-tenant
        // keystore enumeration tools.
        assert!(master_handle(a).0.starts_with("tenant-"));
        assert!(master_handle(a).0.ends_with("/master"));
    }

    #[test]
    fn fresh_tenant_is_not_bootstrapped() {
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        assert!(!mgr.is_bootstrapped(tenant()).unwrap());
    }

    #[test]
    fn bootstrap_persists_master_and_returns_phrase() {
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let t = tenant();
        let result = mgr.bootstrap(t).unwrap();

        // The phrase is 24 words.
        assert_eq!(result.recovery_phrase.word_count(), 24);

        // The keystore now holds the key.
        assert!(mgr.is_bootstrapped(t).unwrap());

        // Unlocking returns the SAME key bytes the bootstrap
        // produced — keystore round-trip is faithful.
        let unlocked = mgr.unlock(t).unwrap();
        assert_eq!(unlocked.as_bytes(), result.master_key.as_bytes());
    }

    #[test]
    fn bootstrap_refuses_to_overwrite_an_existing_key() {
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let t = tenant();
        mgr.bootstrap(t).unwrap();
        let err = mgr.bootstrap(t).unwrap_err();
        assert!(matches!(err, CryptoError::InvalidParameter(_)));
    }

    #[test]
    fn unlock_on_unbound_tenant_surfaces_typed_error() {
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let err = mgr.unlock(tenant()).unwrap_err();
        assert!(matches!(err, CryptoError::InvalidParameter(_)));
    }

    #[test]
    fn restore_with_correct_phrase_reconstructs_the_same_key() {
        // The headline correctness property: phrase + tenant
        // UUID is the SAME input to Argon2id whether you're
        // bootstrapping or restoring, so the master key is
        // bit-identical across the two paths.
        let t = tenant();
        let mgr1 = MasterKeyManager::new(MemoryKeystore::new());
        let initial = mgr1.bootstrap(t).unwrap();

        // Fresh device — empty keystore, restore from phrase.
        let mgr2 = MasterKeyManager::new(MemoryKeystore::new());
        let restored = mgr2
            .restore_from_phrase(&initial.recovery_phrase, t)
            .unwrap();
        assert_eq!(
            restored.as_bytes(),
            initial.master_key.as_bytes(),
            "restore must reconstruct the bit-identical key"
        );
        assert!(mgr2.is_bootstrapped(t).unwrap());
    }

    #[test]
    fn restore_with_wrong_tenant_yields_different_key() {
        // Tenant UUID is the Argon2id salt. Same phrase + a
        // different tenant ID → different key. This is the
        // emergency-envelope safety property: a service
        // provider holding all tenants' phrases can't recover
        // tenant A's data using tenant B's phrase.
        let t_a = tenant();
        let t_b = tenant();
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let bootstrap = mgr.bootstrap(t_a).unwrap();

        let mgr_b = MasterKeyManager::new(MemoryKeystore::new());
        let wrong = mgr_b
            .restore_from_phrase(&bootstrap.recovery_phrase, t_b)
            .unwrap();
        assert_ne!(wrong.as_bytes(), bootstrap.master_key.as_bytes());
    }

    #[test]
    fn restore_replaces_existing_key_for_the_same_tenant() {
        // Operator typed the phrase, so they own the device
        // now. A previous (possibly compromised) key gets
        // overwritten with the correctly-derived one.
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let t = tenant();
        let initial = mgr.bootstrap(t).unwrap();
        // Simulate a different stored key (via direct keystore
        // write — production code would never do this, but
        // the test pins the overwrite semantic).
        let bogus = vec![0xFFu8; 32];
        mgr.keystore()
            .store(&master_handle(t), Zeroizing::new(bogus))
            .unwrap();

        // Restoring from the genuine phrase puts the right
        // key back.
        let restored = mgr
            .restore_from_phrase(&initial.recovery_phrase, t)
            .unwrap();
        assert_eq!(restored.as_bytes(), initial.master_key.as_bytes());
        assert_eq!(
            mgr.unlock(t).unwrap().as_bytes(),
            initial.master_key.as_bytes()
        );
    }

    #[test]
    fn corrupted_keystore_entry_surfaces_typed_error_not_panic() {
        // If the stored bytes are wrong-length (corruption,
        // wrong handle, manual fiddling), unlock returns a
        // typed error rather than panicking in the from_bytes
        // copy.
        let mgr = MasterKeyManager::new(MemoryKeystore::new());
        let t = tenant();
        mgr.keystore()
            .store(&master_handle(t), Zeroizing::new(vec![0xAAu8; 7]))
            .unwrap();
        let err = mgr.unlock(t).unwrap_err();
        assert!(matches!(err, CryptoError::InvalidParameter(_)));
    }
}
