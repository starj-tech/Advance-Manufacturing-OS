use aether_crypto::{verify, VerifyingKey};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::manifest::Manifest;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("publisher key not trusted: {0}")]
    UntrustedPublisher(String),
    #[error("signature mismatch")]
    SignatureMismatch,
    #[error("hash mismatch")]
    HashMismatch,
}

/// Verify a module bundle.
///
/// `bundle_bytes` is the raw, unmodified bundle (e.g. tar.gz). `signature`
/// is the detached ed25519 signature over `Sha256(bundle_bytes) || manifest_bytes`.
/// `trusted_keys` is the per-tenant set of `(publisher_key_id, public_key)`.
pub fn verify_bundle(
    manifest: &Manifest,
    manifest_bytes: &[u8],
    bundle_bytes: &[u8],
    signature: &[u8],
    trusted_keys: &[(String, [u8; 32])],
) -> Result<(), VerifyError> {
    let publisher_id = &manifest.signature.public_key_id;
    let key_bytes = trusted_keys
        .iter()
        .find(|(id, _)| id == publisher_id)
        .map(|(_, k)| k)
        .ok_or_else(|| VerifyError::UntrustedPublisher(publisher_id.clone()))?;

    let vk = VerifyingKey::from_bytes(key_bytes).map_err(|e| VerifyError::Crypto(e.to_string()))?;

    let mut hasher = Sha256::new();
    hasher.update(bundle_bytes);
    let bundle_hash = hasher.finalize();

    let mut signed = Vec::with_capacity(bundle_hash.len() + manifest_bytes.len());
    signed.extend_from_slice(&bundle_hash);
    signed.extend_from_slice(manifest_bytes);

    verify(&vk, &signed, signature).map_err(|_| VerifyError::SignatureMismatch)
}
