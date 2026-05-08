use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, CryptoResult};

const BLIND_INDEX_LEN: usize = 16;

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct BlindIndexKey([u8; 32]);

impl BlindIndexKey {
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlindIndex(pub [u8; BLIND_INDEX_LEN]);

impl BlindIndex {
    pub fn as_bytes(&self) -> &[u8; BLIND_INDEX_LEN] {
        &self.0
    }
}

/// Compute a deterministic blind index over `value` using HMAC-SHA256
/// truncated to 16 bytes. The key MUST be derived from the tenant master
/// key (HKDF "search" context). Server stores the index alongside the
/// ciphertext to support equality lookups without learning plaintext.
///
/// `value` should be normalized (lowercased, trimmed) by the caller.
pub fn blind_index(key: &BlindIndexKey, value: &[u8]) -> CryptoResult<BlindIndex> {
    let mut mac = Hmac::<Sha256>::new_from_slice(&key.0).map_err(|_| CryptoError::BlindIndex)?;
    mac.update(value);
    let tag = mac.finalize().into_bytes();
    let mut out = [0u8; BLIND_INDEX_LEN];
    out.copy_from_slice(&tag[..BLIND_INDEX_LEN]);
    Ok(BlindIndex(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let k = BlindIndexKey::from_bytes([4u8; 32]);
        let a = blind_index(&k, b"alice@example.com").unwrap();
        let b = blind_index(&k, b"alice@example.com").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn key_separation() {
        let k1 = BlindIndexKey::from_bytes([1u8; 32]);
        let k2 = BlindIndexKey::from_bytes([2u8; 32]);
        let a = blind_index(&k1, b"x").unwrap();
        let b = blind_index(&k2, b"x").unwrap();
        assert_ne!(a, b);
    }
}
