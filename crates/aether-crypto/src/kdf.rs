use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{CryptoError, CryptoResult};

/// OWASP-recommended Argon2id parameters (2024). Tuned to ~200ms on a
/// modern laptop.
pub const ARGON2_MEM_KIB: u32 = 64 * 1024; // 64 MiB
pub const ARGON2_TIME_COST: u32 = 3;
pub const ARGON2_PARALLELISM: u32 = 1;

/// 32-byte master key, zeroized on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; 32]);

impl MasterKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterKey(***)")
    }
}

/// Derive a 32-byte master key from a recovery phrase + tenant-bound salt.
///
/// `salt` MUST be at least 16 bytes. We recommend using the tenant_id
/// (UUID, 16 bytes) as salt — that way the same recovery phrase derives
/// different keys per tenant.
pub fn derive_master_key(recovery_phrase: &[u8], salt: &[u8]) -> CryptoResult<MasterKey> {
    if salt.len() < 16 {
        return Err(CryptoError::Kdf("salt < 16 bytes".into()));
    }

    let params = Params::new(ARGON2_MEM_KIB, ARGON2_TIME_COST, ARGON2_PARALLELISM, Some(32))
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut out = [0u8; 32];
    argon
        .hash_password_into(recovery_phrase, salt, &mut out)
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    Ok(MasterKey(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_inputs() {
        let salt = [9u8; 16];
        let a = derive_master_key(b"correct horse battery staple", &salt).unwrap();
        let b = derive_master_key(b"correct horse battery staple", &salt).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn different_salt_yields_different_key() {
        let a = derive_master_key(b"phrase", &[1u8; 16]).unwrap();
        let b = derive_master_key(b"phrase", &[2u8; 16]).unwrap();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn rejects_short_salt() {
        let r = derive_master_key(b"phrase", &[0u8; 8]);
        assert!(matches!(r, Err(CryptoError::Kdf(_))));
    }
}
