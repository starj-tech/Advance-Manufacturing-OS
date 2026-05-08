//! Zero-knowledge cryptography for AETHER-OS.
//!
//! ## Threat model
//! The server (Supabase) is honest-but-curious for non-encrypted data
//! and untrusted for encrypted data (formulas, financials, IP).
//! Master keys never leave the device; we use OS keychain to wrap the
//! per-device blob and Argon2id to derive from a recovery phrase on
//! enrollment.
//!
//! ## Algorithm choices
//! - AEAD: XChaCha20-Poly1305 (24-byte nonce, no nonce-reuse risk)
//! - KDF: Argon2id (m=64MB, t=3, p=1) — OWASP recommended
//! - Asymmetric: X25519 + XSalsa20-Poly1305 via `crypto_box`
//! - Signing: Ed25519 (`ed25519-dalek`)
//! - Blind index: HMAC-SHA256, truncated to 16 bytes
//!
//! See `docs/architecture/crypto.md` for the master key flow.

pub mod aead;
pub mod blind_index;
pub mod kdf;
pub mod keystore;
pub mod signer;

pub use aead::{decrypt, encrypt, Ciphertext, Nonce, AEAD_KEY_LEN};
pub use blind_index::{BlindIndex, BlindIndexKey};
pub use kdf::{derive_master_key, MasterKey, ARGON2_MEM_KIB, ARGON2_PARALLELISM, ARGON2_TIME_COST};
pub use keystore::{KeyHandle, Keystore};
pub use signer::{verify, Signer, SigningError, VerifyingKey};

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("aead failure")]
    Aead,

    #[error("kdf failure: {0}")]
    Kdf(String),

    #[error("blind index failure")]
    BlindIndex,

    #[error("keystore: {0}")]
    Keystore(String),

    #[error("signature: {0}")]
    Signature(String),

    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type CryptoResult<T> = Result<T, CryptoError>;
