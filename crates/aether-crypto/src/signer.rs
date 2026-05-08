use ed25519_dalek::{
    Signature, Signer as Ed25519Signer, SigningKey, Verifier, VerifyingKey as Ed25519VerifyingKey,
};
use rand_core::OsRng;
use thiserror::Error;

use crate::CryptoError;

#[derive(Debug, Error)]
pub enum SigningError {
    #[error("invalid key")]
    InvalidKey,
    #[error("invalid signature")]
    InvalidSignature,
}

impl From<SigningError> for CryptoError {
    fn from(e: SigningError) -> Self {
        CryptoError::Signature(e.to_string())
    }
}

#[derive(Clone)]
pub struct Signer {
    inner: SigningKey,
}

impl Signer {
    pub fn generate() -> Self {
        Self {
            inner: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_bytes(b: &[u8; 32]) -> Self {
        Self {
            inner: SigningKey::from_bytes(b),
        }
    }

    pub fn public_key(&self) -> VerifyingKey {
        VerifyingKey {
            inner: self.inner.verifying_key(),
        }
    }

    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        self.inner.sign(msg).to_bytes().to_vec()
    }
}

#[derive(Clone)]
pub struct VerifyingKey {
    inner: Ed25519VerifyingKey,
}

impl VerifyingKey {
    pub fn from_bytes(b: &[u8; 32]) -> Result<Self, SigningError> {
        Ed25519VerifyingKey::from_bytes(b)
            .map(|inner| Self { inner })
            .map_err(|_| SigningError::InvalidKey)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }
}

pub fn verify(vk: &VerifyingKey, msg: &[u8], sig: &[u8]) -> Result<(), SigningError> {
    let sig = Signature::from_slice(sig).map_err(|_| SigningError::InvalidSignature)?;
    vk.inner
        .verify(msg, &sig)
        .map_err(|_| SigningError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let s = Signer::generate();
        let msg = b"manifest-bytes";
        let sig = s.sign(msg);
        let vk = s.public_key();
        assert!(verify(&vk, msg, &sig).is_ok());
    }

    #[test]
    fn rejects_tampered_message() {
        let s = Signer::generate();
        let sig = s.sign(b"original");
        let vk = s.public_key();
        assert!(verify(&vk, b"tampered", &sig).is_err());
    }
}
