use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, XChaCha20Poly1305,
};
use serde::{Deserialize, Serialize};

use crate::{CryptoError, CryptoResult};

pub const AEAD_KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ciphertext {
    pub nonce: Vec<u8>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Nonce(pub [u8; NONCE_LEN]);

/// Encrypt `plaintext` with the 32-byte symmetric `key` using XChaCha20-Poly1305.
/// Generates a fresh random 24-byte nonce.
pub fn encrypt(key: &[u8; AEAD_KEY_LEN], plaintext: &[u8]) -> CryptoResult<Ciphertext> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let bytes = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| CryptoError::Aead)?;
    Ok(Ciphertext {
        nonce: nonce.to_vec(),
        bytes,
    })
}

/// Decrypt the given Ciphertext.
pub fn decrypt(key: &[u8; AEAD_KEY_LEN], ct: &Ciphertext) -> CryptoResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    if ct.nonce.len() != NONCE_LEN {
        return Err(CryptoError::Aead);
    }
    let nonce = chacha20poly1305::XNonce::from_slice(&ct.nonce);
    cipher
        .decrypt(nonce, ct.bytes.as_ref())
        .map_err(|_| CryptoError::Aead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = [7u8; AEAD_KEY_LEN];
        let pt = b"top-secret manufacturing recipe";
        let ct = encrypt(&key, pt).unwrap();
        assert_ne!(&ct.bytes[..], &pt[..]);
        let pt2 = decrypt(&key, &ct).unwrap();
        assert_eq!(&pt2[..], &pt[..]);
    }

    #[test]
    fn wrong_key_fails() {
        let key = [1u8; AEAD_KEY_LEN];
        let key2 = [2u8; AEAD_KEY_LEN];
        let pt = b"hello";
        let ct = encrypt(&key, pt).unwrap();
        assert!(decrypt(&key2, &ct).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = [3u8; AEAD_KEY_LEN];
        let mut ct = encrypt(&key, b"hello").unwrap();
        ct.bytes[0] ^= 0x01;
        assert!(decrypt(&key, &ct).is_err());
    }
}
