//! `FrameSealer` — abstraction over "encrypt frame bytes before
//! durable storage". Used by the pipeline (V4) when capturing a
//! frame whose `PrivacyClass::Sensitive` flag is set.
//!
//! ## Why an abstraction
//! Production wires `CryptoSealer` (below) which uses
//! `aether_crypto::envelope::seal` with a per-tenant DATA_DEK. But:
//!
//!   * Tests need to verify "sealing happened" without standing up
//!     a real key chain. `MockSealer` returns bytes prefixed with
//!     a known marker so tests can assert opaquely.
//!   * Edge cases (HSM-backed seal, remote-attested seal) might
//!     want different sealing strategies. Keeping the trait open
//!     means swapping is a one-line `Arc<dyn FrameSealer>` change.
//!
//! ## What this is NOT
//! It's NOT an opaque-blob format. The pipeline doesn't introspect
//! the sealed bytes — they just go to durable storage as-is. Decode
//! happens client-side via the same `aether_crypto::envelope::open`
//! after the bytes are pulled back. This crate doesn't own decryption
//! because the read path lives in a different runtime (sync engine →
//! UI render).

use aether_crypto::SubKey;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SealError {
    #[error("seal failed: {0}")]
    Crypto(String),
}

#[async_trait]
pub trait FrameSealer: Send + Sync {
    /// Encrypt `plaintext` and return the sealed bytes. Idempotent —
    /// calling twice with the same input is safe (produces different
    /// ciphertexts due to nonce, but each decrypts back to the same
    /// plaintext).
    async fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SealError>;
}

/// Production sealer: wraps `aether_crypto::envelope::seal` with the
/// tenant's DATA_DEK. The DEK is held by reference inside the sealer
/// for the lifetime of the camera session — it's already a
/// `Zeroize`-protected `SubKey` so the memory hygiene is handled.
pub struct CryptoSealer {
    dek: SubKey,
}

impl CryptoSealer {
    pub fn new(dek: SubKey) -> Self {
        Self { dek }
    }
}

#[async_trait]
impl FrameSealer for CryptoSealer {
    async fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SealError> {
        // `envelope::seal` is synchronous CPU work (chacha20-poly1305
        // on the current thread); we await `async` only for trait
        // shape consistency, not because we yield.
        aether_crypto::envelope::seal(&self.dek, plaintext)
            .map_err(|e| SealError::Crypto(format!("{e}")))
    }
}

/// In-process sealer for tests. Prepends `MOCK_SEAL_PREFIX` so a
/// test can assert "sealing happened" without going through real
/// AEAD. Default-friendly: `MockSealer::new()` works in any test.
pub struct MockSealer {
    prefix: Vec<u8>,
    /// If true, every `seal` call returns an error — used to test
    /// the pipeline's failure path without real crypto failures.
    fail: bool,
}

pub const MOCK_SEAL_PREFIX: &[u8] = b"SEALED:";

impl MockSealer {
    pub fn new() -> Self {
        Self {
            prefix: MOCK_SEAL_PREFIX.to_vec(),
            fail: false,
        }
    }

    /// Builder: make every subsequent `seal` call fail. Lets the
    /// pipeline test surface the SealError path without
    /// constructing a broken DEK.
    pub fn always_fail(mut self) -> Self {
        self.fail = true;
        self
    }
}

impl Default for MockSealer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FrameSealer for MockSealer {
    async fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, SealError> {
        if self.fail {
            return Err(SealError::Crypto("mock sealer in always_fail mode".into()));
        }
        let mut out = self.prefix.clone();
        out.extend_from_slice(plaintext);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_sealer_prepends_known_prefix() {
        let s = MockSealer::new();
        let sealed = s.seal(b"hello").await.unwrap();
        assert!(sealed.starts_with(MOCK_SEAL_PREFIX));
        assert!(sealed.ends_with(b"hello"));
    }

    #[tokio::test]
    async fn mock_sealer_always_fail_mode_surfaces_crypto_error() {
        // Used by the pipeline tests to exercise the SealError path
        // without rigging a broken DEK.
        let s = MockSealer::new().always_fail();
        let err = s.seal(b"hello").await.unwrap_err();
        assert!(matches!(err, SealError::Crypto(_)));
    }

    #[tokio::test]
    async fn crypto_sealer_round_trips_via_envelope_open() {
        // Production-shape contract: a CryptoSealer's output must
        // be decryptable by `aether_crypto::envelope::open` with
        // the same DEK. If this ever drifts the read path is broken.
        use aether_crypto::derive::SUBKEY_LEN;
        use aether_crypto::kdf::MasterKey;

        let master = MasterKey::from_bytes([7u8; SUBKEY_LEN]);
        let dek = master.derive_data_dek().unwrap();
        let sealer = CryptoSealer::new(dek.clone());

        let pt = b"sensitive frame bytes 12345";
        let sealed = sealer.seal(pt).await.unwrap();

        // The sealed bytes are NOT the plaintext.
        assert_ne!(&sealed[..], pt);
        // And the open() path recovers the original.
        let recovered = aether_crypto::envelope::open(&dek, &sealed).unwrap();
        assert_eq!(recovered, pt);
    }

    #[tokio::test]
    async fn distinct_seal_calls_produce_distinct_ciphertexts() {
        // Nonce-uniqueness property bubbling up through the trait —
        // two seals of identical input must differ byte-wise. Tested
        // at the envelope layer but worth re-pinning here so a future
        // FrameSealer impl that ignores nonces breaks loudly.
        use aether_crypto::derive::SUBKEY_LEN;
        use aether_crypto::kdf::MasterKey;
        let master = MasterKey::from_bytes([3u8; SUBKEY_LEN]);
        let sealer = CryptoSealer::new(master.derive_data_dek().unwrap());
        let a = sealer.seal(b"identical").await.unwrap();
        let b = sealer.seal(b"identical").await.unwrap();
        assert_ne!(a, b);
    }
}
