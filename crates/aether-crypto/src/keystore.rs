//! OS keychain integration.
//!
//! - macOS: Keychain via `security-framework`
//! - Windows: DPAPI / Credential Manager via `windows-rs`
//! - Linux: Secret Service via `secret-service`
//! - Fallback: Tauri Stronghold (IOTA Stronghold) — encrypted file blob
//!
//! For PR #1 we ship the trait and a `MemoryKeystore` test impl. The
//! per-OS implementations land in PR #2 (zero-knowledge layer).

use crate::CryptoResult;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KeyHandle(pub String);

impl KeyHandle {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

pub trait Keystore: Send + Sync {
    fn store(&self, handle: &KeyHandle, secret: Zeroizing<Vec<u8>>) -> CryptoResult<()>;
    fn load(&self, handle: &KeyHandle) -> CryptoResult<Option<Zeroizing<Vec<u8>>>>;
    fn delete(&self, handle: &KeyHandle) -> CryptoResult<()>;
}

/// In-memory keystore for unit/integration tests.
#[derive(Default)]
pub struct MemoryKeystore {
    inner: Mutex<HashMap<KeyHandle, Vec<u8>>>,
}

impl MemoryKeystore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<KeyHandle, Vec<u8>>> {
        self.inner.lock().expect("MemoryKeystore mutex poisoned")
    }
}

impl Keystore for MemoryKeystore {
    fn store(&self, handle: &KeyHandle, secret: Zeroizing<Vec<u8>>) -> CryptoResult<()> {
        self.lock().insert(handle.clone(), secret.to_vec());
        Ok(())
    }

    fn load(&self, handle: &KeyHandle) -> CryptoResult<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.lock().get(handle).cloned().map(Zeroizing::new))
    }

    fn delete(&self, handle: &KeyHandle) -> CryptoResult<()> {
        self.lock().remove(handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_load_delete() {
        let ks = MemoryKeystore::new();
        let h = KeyHandle::new("tenant-a/master");
        let secret = Zeroizing::new(b"super-secret".to_vec());

        ks.store(&h, secret.clone()).unwrap();
        let got = ks.load(&h).unwrap().unwrap();
        assert_eq!(got.as_slice(), b"super-secret");

        ks.delete(&h).unwrap();
        assert!(ks.load(&h).unwrap().is_none());
    }
}
