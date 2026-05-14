//! HKDF-SHA256 sub-key derivation from a tenant [`MasterKey`].
//!
//! ## Why a layer above the master key?
//! The tenant master key is the root of every encryption operation in
//! AETHER-OS — but using it directly for every purpose is a textbook
//! key-reuse hazard:
//!
//!   - The same key would AEAD-encrypt rows AND HMAC a blind index AND
//!     authenticate sync messages, which means a single side-channel
//!     leak compromises all three.
//!   - Rotating the data encryption key would require rotating every
//!     other use simultaneously.
//!   - Audit becomes harder: "which key did this ciphertext use?" has
//!     no good answer.
//!
//! HKDF-Expand with domain-separated `info` strings produces a fresh
//! 32-byte sub-key per purpose. The master key is the IKM, the salt is
//! a fixed context label (so derivation is deterministic across runs
//! on the same tenant), and `info` carries the purpose tag.
//!
//! ## Domain separation
//! Every sub-key has a unique context literal:
//!
//! ```text
//!   "aether-os/v1/data-dek"         → AEAD per-row payload key
//!   "aether-os/v1/blind-index"      → HMAC-SHA256 deterministic index
//!   "aether-os/v1/wrap"             → AEAD-wrap key for sub-secrets
//!   "aether-os/v1/module/<id>"      → module-scoped sub-key
//! ```
//!
//! The `v1` segment lets us migrate the derivation scheme (e.g. swap
//! HKDF for KMAC-128) without touching old ciphertexts: bump the
//! segment, re-derive the new keys, re-encrypt under them.

use crate::kdf::MasterKey;
use crate::{CryptoError, CryptoResult};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Length of every sub-key we derive. 32 bytes feeds XChaCha20-Poly1305
/// (which needs exactly 32) and HMAC-SHA256 (which accepts arbitrary
/// but maxes its security at 32). Keeping a single length avoids
/// branchy "which size?" logic in every caller.
pub const SUBKEY_LEN: usize = 32;

/// HKDF salt — a stable context literal, not a secret. The combination
/// of (salt, info) is what guarantees per-purpose uniqueness; the salt
/// alone identifies the AETHER-OS namespace so the same master key
/// could in theory be reused by a third party with a different salt
/// without collision.
const HKDF_SALT: &[u8] = b"aether-os/hkdf/v1";

// Domain-separation labels. Adding a new label is a one-line change
// here; the helper methods below use the const so there's exactly one
// place to audit.
const INFO_DATA_DEK: &[u8] = b"aether-os/v1/data-dek";
const INFO_BLIND_INDEX: &[u8] = b"aether-os/v1/blind-index";
const INFO_WRAP: &[u8] = b"aether-os/v1/wrap";
const INFO_MODULE_PREFIX: &[u8] = b"aether-os/v1/module/";

/// A purpose-bound 32-byte sub-key. Distinct from `MasterKey` so the
/// type system catches "passed the master key where a DEK was wanted".
/// Zeroize + ZeroizeOnDrop scrubs the bytes the moment the value
/// drops, matching how `MasterKey` itself behaves.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SubKey([u8; SUBKEY_LEN]);

impl SubKey {
    pub fn as_bytes(&self) -> &[u8; SUBKEY_LEN] {
        &self.0
    }

    pub fn from_bytes(b: [u8; SUBKEY_LEN]) -> Self {
        Self(b)
    }
}

impl std::fmt::Debug for SubKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SubKey(***)")
    }
}

/// Core HKDF-Expand step. All convenience wrappers below call into this.
/// Returns `CryptoError::Kdf` only when HKDF's expand rejects the
/// requested output size (which it can't at 32 bytes, but we surface
/// the error path anyway for forward-compatibility).
fn expand(master: &MasterKey, info: &[u8]) -> CryptoResult<SubKey> {
    let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), master.as_bytes());
    let mut out = [0u8; SUBKEY_LEN];
    hk.expand(info, &mut out)
        .map_err(|e| CryptoError::Kdf(format!("hkdf expand: {e}")))?;
    Ok(SubKey(out))
}

impl MasterKey {
    /// Per-row AEAD encryption key. Use this — never `MasterKey`
    /// directly — for `aether-crypto::encrypt` / `decrypt` calls that
    /// touch sensitive columns.
    pub fn derive_data_dek(&self) -> CryptoResult<SubKey> {
        expand(self, INFO_DATA_DEK)
    }

    /// HMAC key for `aether-crypto::blind_index`. Same key across all
    /// rows so equality lookups work; never reuse for AEAD.
    pub fn derive_blind_index_key(&self) -> CryptoResult<SubKey> {
        expand(self, INFO_BLIND_INDEX)
    }

    /// Key-wrapping key — used when one tenant secret protects another
    /// (e.g. wrapping a per-document Automerge key). Distinct from the
    /// device-bound wrap key in `SessionVault`.
    pub fn derive_wrap_key(&self) -> CryptoResult<SubKey> {
        expand(self, INFO_WRAP)
    }

    /// Module-scoped sub-key. Modules registered through
    /// `aether-modules` get their own deterministic key keyed by
    /// module id, so a compromised module can't derive any other
    /// module's secrets (assuming each module enforces correct usage).
    ///
    /// `module_id` is appended to a constant prefix to keep the
    /// namespace separate from system sub-keys.
    pub fn derive_module_subkey(&self, module_id: &str) -> CryptoResult<SubKey> {
        let mut info = Vec::with_capacity(INFO_MODULE_PREFIX.len() + module_id.len());
        info.extend_from_slice(INFO_MODULE_PREFIX);
        info.extend_from_slice(module_id.as_bytes());
        expand(self, &info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aead::{decrypt, encrypt};
    use crate::blind_index::{blind_index, BlindIndexKey};
    use crate::recovery::{derive_master_key_for_tenant, RecoveryPhrase};
    use uuid::Uuid;

    fn master() -> MasterKey {
        MasterKey::from_bytes([3u8; SUBKEY_LEN])
    }

    #[test]
    fn deterministic_for_same_master() {
        let m = master();
        let a = m.derive_data_dek().unwrap();
        let b = m.derive_data_dek().unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn different_master_yields_different_dek() {
        let m1 = MasterKey::from_bytes([1u8; SUBKEY_LEN]);
        let m2 = MasterKey::from_bytes([2u8; SUBKEY_LEN]);
        let k1 = m1.derive_data_dek().unwrap();
        let k2 = m2.derive_data_dek().unwrap();
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn domain_separation_between_purposes() {
        // The whole point: same master, different info → distinct keys.
        // A leak of the DEK must not let an attacker recover the blind
        // index key or the wrap key.
        let m = master();
        let dek = m.derive_data_dek().unwrap();
        let bi = m.derive_blind_index_key().unwrap();
        let wrap = m.derive_wrap_key().unwrap();
        assert_ne!(dek.as_bytes(), bi.as_bytes());
        assert_ne!(dek.as_bytes(), wrap.as_bytes());
        assert_ne!(bi.as_bytes(), wrap.as_bytes());
    }

    #[test]
    fn module_subkey_namespace_isolated() {
        let m = master();
        let qc = m.derive_module_subkey("com.aether.qc-spc").unwrap();
        let pmnt = m
            .derive_module_subkey("com.aether.predictive-maintenance")
            .unwrap();
        let dek = m.derive_data_dek().unwrap();

        assert_ne!(qc.as_bytes(), pmnt.as_bytes(), "per-module isolation");
        // Modules also distinct from system sub-keys — a compromised
        // module key must not collide with the DEK.
        assert_ne!(qc.as_bytes(), dek.as_bytes());
    }

    #[test]
    fn module_subkey_is_deterministic() {
        let m = master();
        let a = m.derive_module_subkey("com.aether.qc-spc").unwrap();
        let b = m.derive_module_subkey("com.aether.qc-spc").unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn dek_round_trips_with_aead() {
        // Whole-stack smoke: derive DEK from master, encrypt+decrypt with
        // it, plaintext survives.
        let m = master();
        let dek = m.derive_data_dek().unwrap();
        let pt = b"recipe: 250g flour, 2 eggs, 1 cup milk";
        let ct = encrypt(dek.as_bytes(), pt).unwrap();
        let recovered = decrypt(dek.as_bytes(), &ct).unwrap();
        assert_eq!(recovered, pt);
    }

    #[test]
    fn blind_index_key_works_with_blind_index_hmac() {
        // The derived sub-key is shaped correctly for blind_index — i.e.
        // 32 bytes, accepted by BlindIndexKey, yields a stable index.
        let m = master();
        let bi_sub = m.derive_blind_index_key().unwrap();
        let bi_key = BlindIndexKey::from_bytes(*bi_sub.as_bytes());

        let a = blind_index(&bi_key, b"alice@example.com").unwrap();
        let b = blind_index(&bi_key, b"alice@example.com").unwrap();
        let c = blind_index(&bi_key, b"bob@example.com").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn full_chain_phrase_to_dek_to_ciphertext() {
        // End-to-end: BIP39 recovery phrase → tenant master key → DEK
        // → AEAD round-trip. This is the actual production wiring
        // (modulo OS keychain) so it deserves a smoke test in-crate.
        let phrase = RecoveryPhrase::generate().unwrap();
        let tenant = Uuid::new_v4();
        let master = derive_master_key_for_tenant(&phrase, tenant).unwrap();
        let dek = master.derive_data_dek().unwrap();

        let plaintext = b"materials.name = 'top-secret food coloring batch 42'";
        let ct = encrypt(dek.as_bytes(), plaintext).unwrap();
        let recovered = decrypt(dek.as_bytes(), &ct).unwrap();
        assert_eq!(recovered, plaintext);

        // And a different tenant, even with the SAME recovery phrase,
        // gets a different DEK so its ciphertexts wouldn't decrypt.
        let other_tenant = Uuid::new_v4();
        let other_master = derive_master_key_for_tenant(&phrase, other_tenant).unwrap();
        let other_dek = other_master.derive_data_dek().unwrap();
        assert_ne!(dek.as_bytes(), other_dek.as_bytes());
        assert!(
            decrypt(other_dek.as_bytes(), &ct).is_err(),
            "tenant isolation must hold across the full crypto stack"
        );
    }

    #[test]
    fn debug_does_not_leak_subkey_bytes() {
        let k = SubKey::from_bytes([42u8; SUBKEY_LEN]);
        let dbg = format!("{k:?}");
        assert!(dbg.contains("***"));
        assert!(!dbg.contains("42"));
    }
}
