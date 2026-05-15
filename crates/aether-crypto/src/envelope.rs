//! Sealed-envelope wire format for AEAD payloads at rest / in transit.
//!
//! Where `aead::encrypt` returns a `Ciphertext { nonce, bytes }` struct,
//! callers eventually need a single contiguous `Vec<u8>` they can stash
//! in a column, the sync outbox, a keystore blob, or an HTTP request
//! body. `envelope::{seal, open}` is the canonical bridge: one function
//! to produce the bytes, one to consume them, with a 1-byte version
//! prefix so we can migrate the AEAD scheme later without re-encrypting
//! historical data en-masse.
//!
//! ## Wire format (v1)
//! ```text
//!   byte 0           : VERSION = 0x01
//!   bytes 1..25      : XChaCha20-Poly1305 nonce (24 bytes)
//!   bytes 25..end    : ciphertext (includes Poly1305 tag)
//! ```
//!
//! Because XChaCha20-Poly1305 always uses a 24-byte nonce, the format
//! needs no length prefix — the ciphertext consumes the remainder. A
//! future v2 (e.g. AES-256-GCM with a 12-byte nonce, or an AAD-bearing
//! scheme) bumps the version byte and the open() dispatcher learns the
//! new offsets. v1 ciphertexts stay readable forever.
//!
//! ## Why the [`SubKey`] type rather than a raw byte array?
//! Type-safety. `seal(&master_key.as_bytes()...)` is a footgun — the
//! master key is the wrap-of-wraps root and must never AEAD a row. By
//! taking `&SubKey`, the only way to call `seal` is through a
//! purpose-bound `MasterKey::derive_*()` first, which the audit trail
//! exposes as a deliberate sub-key choice.
//!
//! ## Failure mode
//! Every failure — wrong key, tampered byte, truncated buffer, unknown
//! version — surfaces as [`CryptoError::Aead`]. This matches how
//! production attackers can probe the API: they don't get to distinguish
//! "format error" from "tag mismatch", which would otherwise be a
//! padding-oracle-shaped side channel.

use crate::aead::{decrypt, encrypt, Ciphertext, NONCE_LEN};
use crate::derive::SubKey;
use crate::{CryptoError, CryptoResult};

/// Current envelope version. Bumping this is how we migrate the AEAD
/// scheme; `open` dispatches on the leading byte so prior versions stay
/// decryptable forever (or until we explicitly drop support).
const VERSION_V1: u8 = 0x01;

/// `1 byte version + 24 byte nonce` — the minimum header size before
/// any ciphertext bytes appear.
const HEADER_LEN_V1: usize = 1 + NONCE_LEN;

/// Encrypt `plaintext` under `key` and return a self-describing byte
/// blob suitable for column storage, outbox queueing, or HTTP transport.
pub fn seal(key: &SubKey, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
    let ct: Ciphertext = encrypt(key.as_bytes(), plaintext)?;
    // The aead module guarantees NONCE_LEN-byte nonces for
    // XChaCha20-Poly1305; assert so a future cipher swap can't silently
    // produce an undersized header.
    if ct.nonce.len() != NONCE_LEN {
        return Err(CryptoError::Aead);
    }
    let mut out = Vec::with_capacity(HEADER_LEN_V1 + ct.bytes.len());
    out.push(VERSION_V1);
    out.extend_from_slice(&ct.nonce);
    out.extend_from_slice(&ct.bytes);
    Ok(out)
}

/// Parse a sealed envelope and decrypt to plaintext. Any structural or
/// cryptographic failure collapses to [`CryptoError::Aead`] to deny
/// attackers a side channel.
pub fn open(key: &SubKey, payload: &[u8]) -> CryptoResult<Vec<u8>> {
    if payload.len() < HEADER_LEN_V1 {
        return Err(CryptoError::Aead);
    }
    match payload[0] {
        VERSION_V1 => {
            let nonce = payload[1..1 + NONCE_LEN].to_vec();
            let bytes = payload[1 + NONCE_LEN..].to_vec();
            let ct = Ciphertext { nonce, bytes };
            decrypt(key.as_bytes(), &ct)
        }
        _ => Err(CryptoError::Aead),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::SUBKEY_LEN;
    use crate::kdf::MasterKey;

    fn dek() -> SubKey {
        // Going through a real MasterKey::derive_data_dek would also
        // work, but that's exercised by the derive.rs tests. Here we
        // want a stable, in-test key.
        SubKey::from_bytes([9u8; SUBKEY_LEN])
    }

    #[test]
    fn seal_open_round_trips() {
        let k = dek();
        let pt = b"materials.batch_no=A42; recipe=top-secret";
        let blob = seal(&k, pt).unwrap();
        let recovered = open(&k, &blob).unwrap();
        assert_eq!(recovered, pt);
    }

    #[test]
    fn seal_emits_versioned_header() {
        let blob = seal(&dek(), b"hi").unwrap();
        assert_eq!(blob[0], VERSION_V1, "leading byte is the version tag");
        assert!(
            blob.len() >= HEADER_LEN_V1,
            "blob carries at least the header"
        );
    }

    #[test]
    fn wrong_key_fails() {
        let blob = seal(&dek(), b"hello").unwrap();
        let other = SubKey::from_bytes([1u8; SUBKEY_LEN]);
        assert!(matches!(open(&other, &blob), Err(CryptoError::Aead)));
    }

    #[test]
    fn tampered_byte_fails() {
        let mut blob = seal(&dek(), b"hello").unwrap();
        // Flip a bit inside the ciphertext body, past the nonce. AEAD
        // tag verification must reject.
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(matches!(open(&dek(), &blob), Err(CryptoError::Aead)));
    }

    #[test]
    fn tampered_nonce_fails() {
        let mut blob = seal(&dek(), b"hello").unwrap();
        // Flip a bit inside the nonce window. The cipher derives a
        // different keystream and tag verification fails.
        blob[5] ^= 0x01;
        assert!(matches!(open(&dek(), &blob), Err(CryptoError::Aead)));
    }

    #[test]
    fn truncated_envelope_fails() {
        let blob = seal(&dek(), b"hello").unwrap();
        // Header-only blob — no ciphertext at all. Should still parse
        // but fail AEAD because there's no Poly1305 tag.
        let header_only = &blob[..HEADER_LEN_V1];
        assert!(matches!(open(&dek(), header_only), Err(CryptoError::Aead)));

        // Shorter than the header itself.
        let stub = &blob[..5];
        assert!(matches!(open(&dek(), stub), Err(CryptoError::Aead)));
    }

    #[test]
    fn empty_payload_fails() {
        assert!(matches!(open(&dek(), &[]), Err(CryptoError::Aead)));
    }

    #[test]
    fn unknown_version_fails() {
        let mut blob = seal(&dek(), b"hello").unwrap();
        blob[0] = 0xFE; // not a known version
        assert!(matches!(open(&dek(), &blob), Err(CryptoError::Aead)));
    }

    #[test]
    fn nonce_is_unique_per_seal() {
        // XChaCha20-Poly1305 fetches its own random nonce, so two seals
        // of identical plaintext under the same key MUST differ
        // byte-wise. This is the load-bearing property that lets us
        // reuse one DEK across an entire tenant's rows.
        let k = dek();
        let a = seal(&k, b"identical-plaintext").unwrap();
        let b = seal(&k, b"identical-plaintext").unwrap();
        assert_ne!(a, b, "fresh nonce per seal");

        // But both decrypt to the same thing.
        assert_eq!(open(&k, &a).unwrap(), open(&k, &b).unwrap());
    }

    #[test]
    fn seals_arbitrary_length_payloads() {
        let k = dek();
        for size in [0usize, 1, 17, 256, 4096] {
            let pt = vec![0xABu8; size];
            let blob = seal(&k, &pt).unwrap();
            let back = open(&k, &blob).unwrap();
            assert_eq!(back.len(), size, "round-trip preserves length");
            assert_eq!(back, pt, "round-trip preserves bytes (len={size})");
        }
    }

    #[test]
    fn full_chain_master_to_dek_to_envelope() {
        // The actual production wiring: MasterKey -> SubKey via derive,
        // then SubKey through seal/open. Smoke that the types compose.
        let master = MasterKey::from_bytes([5u8; SUBKEY_LEN]);
        let dek = master.derive_data_dek().unwrap();
        let pt = b"work_order #42 status=in_progress";
        let blob = seal(&dek, pt).unwrap();
        assert_eq!(open(&dek, &blob).unwrap(), pt);
    }
}
