//! Outbox payload encryption — zero-knowledge bridge between
//! `aether-sync` and `aether-crypto`.
//!
//! ## Why this lives in `aether-sync`
//! The outbox carries `payload: Vec<u8>` + `encrypted: bool` as opaque
//! data. The crypto layer doesn't know about sync; the sync layer
//! doesn't choose what to encrypt. This module is the one place that
//! orchestrates "wrap the payload, flip the flag" in production and
//! its inverse on the receive side.
//!
//! ## What goes through the envelope vs. stays plaintext
//! Only the `payload` bytes (the row delta JSON or CRDT op) are sealed.
//! Server-routing metadata — `entity`, `entity_id`, `op`, `hlc_ts`,
//! `parent_hlc`, `op_id` — stays plaintext so the server can:
//!   * index by entity/entity_id for `pull_since` queries,
//!   * order by HLC for LWW reconciliation,
//!   * dedupe by op_id without the per-tenant DEK.
//!
//! This is the zero-knowledge tradeoff: the server learns *which*
//! tenants are syncing *which* entities at *which* times, but not the
//! contents of any row. For tenant identity / row-presence concealment
//! we'd need an oblivious-transport layer above this (out of scope for
//! v1; see `docs/architecture/threat-model.md`).
//!
//! ## DEK source
//! Callers MUST pass the tenant `DATA_DEK` derived via
//! `MasterKey::derive_data_dek()` (see `aether-crypto::derive`). The
//! type system enforces "no using the master key directly" by taking
//! `&SubKey` rather than raw bytes.

use crate::outbox::OutboxEntry;
use aether_crypto::envelope::{open, seal};
use aether_crypto::{CryptoError, SubKey};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PayloadError {
    /// Encryption requested on an entry whose `encrypted` flag is
    /// already set. Refusing the double-wrap is intentional: a
    /// double-encrypted payload is unrecoverable without remembering
    /// the wrap order, and the bug usually indicates a control-flow
    /// mistake (e.g. enqueuing an entry twice through the encrypt
    /// pipeline).
    #[error("entry already encrypted")]
    AlreadyEncrypted,
    /// Decryption requested on a plaintext entry. Refusing this surfaces
    /// the precondition violation explicitly rather than handing back
    /// the raw payload bytes and pretending we authenticated them —
    /// caller code that didn't expect a plaintext entry should crash
    /// loudly so the bug is found in test, not production.
    #[error("entry is not encrypted")]
    NotEncrypted,
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
}

/// Wrap `entry.payload` under the tenant DEK. Consumes the entry and
/// returns a new one with `encrypted=true` and the sealed envelope as
/// payload. Metadata (entity, hlc, op_id, …) is unchanged so the server
/// can still route and order.
pub fn encrypt_entry(mut entry: OutboxEntry, dek: &SubKey) -> Result<OutboxEntry, PayloadError> {
    if entry.encrypted {
        return Err(PayloadError::AlreadyEncrypted);
    }
    let sealed = seal(dek, &entry.payload)?;
    entry.payload = sealed;
    entry.encrypted = true;
    Ok(entry)
}

/// Decrypt the sealed payload of an encrypted entry. The entry itself
/// is borrowed and unmodified — callers usually decrypt-then-apply, so
/// keeping the entry intact lets the applier observe the metadata
/// alongside the plaintext.
pub fn decrypt_entry(entry: &OutboxEntry, dek: &SubKey) -> Result<Vec<u8>, PayloadError> {
    if !entry.encrypted {
        return Err(PayloadError::NotEncrypted);
    }
    Ok(open(dek, &entry.payload)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Op;
    use aether_core::Hlc;
    use aether_crypto::derive::SUBKEY_LEN;
    use aether_crypto::kdf::MasterKey;

    fn dek() -> SubKey {
        let master = MasterKey::from_bytes([4u8; SUBKEY_LEN]);
        master.derive_data_dek().unwrap()
    }

    fn plaintext_entry(payload: &[u8]) -> OutboxEntry {
        OutboxEntry {
            op_id: "op-1".into(),
            entity: "materials".into(),
            entity_id: "mat-42".into(),
            op: Op::Update,
            payload: payload.to_vec(),
            hlc_ts: Hlc::new(1, 0, "node-a"),
            parent_hlc: None,
            encrypted: false,
        }
    }

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let pt = b"{\"qty\":42,\"batch\":\"A-007\"}";
        let entry = plaintext_entry(pt);
        let encrypted = encrypt_entry(entry, &dek()).unwrap();
        assert!(encrypted.encrypted);
        // Server-visible payload is ciphertext, not the plaintext.
        assert_ne!(&encrypted.payload, pt);

        let recovered = decrypt_entry(&encrypted, &dek()).unwrap();
        assert_eq!(&recovered, pt);
    }

    #[test]
    fn metadata_remains_plaintext_after_encrypt() {
        // The server needs entity/entity_id/hlc/op visible to do its
        // routing. Encryption MUST NOT touch these.
        let entry = plaintext_entry(b"secret-formula");
        let original_id = entry.op_id.clone();
        let original_entity = entry.entity.clone();
        let original_eid = entry.entity_id.clone();
        let original_hlc = entry.hlc_ts.clone();
        let original_op = entry.op.clone();

        let encrypted = encrypt_entry(entry, &dek()).unwrap();
        assert_eq!(encrypted.op_id, original_id);
        assert_eq!(encrypted.entity, original_entity);
        assert_eq!(encrypted.entity_id, original_eid);
        assert_eq!(encrypted.hlc_ts, original_hlc);
        assert_eq!(encrypted.op, original_op);
    }

    #[test]
    fn payload_does_not_contain_plaintext_bytes() {
        // Sanity belt-and-braces: a literal string in the plaintext
        // must not appear in the sealed payload.
        let pt = b"unique-marker-string-for-this-test";
        let entry = plaintext_entry(pt);
        let encrypted = encrypt_entry(entry, &dek()).unwrap();
        let marker_in_ct = encrypted
            .payload
            .windows(pt.len())
            .any(|w| w == pt.as_slice());
        assert!(!marker_in_ct, "plaintext marker leaked into ciphertext");
    }

    #[test]
    fn decrypt_with_wrong_dek_fails() {
        let entry = plaintext_entry(b"hello");
        let encrypted = encrypt_entry(entry, &dek()).unwrap();
        let other_master = MasterKey::from_bytes([5u8; SUBKEY_LEN]);
        let other_dek = other_master.derive_data_dek().unwrap();
        assert!(matches!(
            decrypt_entry(&encrypted, &other_dek),
            Err(PayloadError::Crypto(CryptoError::Aead))
        ));
    }

    #[test]
    fn encrypt_already_encrypted_is_rejected() {
        // Double-wrap protection — explained in the error docstring.
        let entry = plaintext_entry(b"hello");
        let once = encrypt_entry(entry, &dek()).unwrap();
        let err = encrypt_entry(once, &dek()).unwrap_err();
        assert!(matches!(err, PayloadError::AlreadyEncrypted));
    }

    #[test]
    fn decrypt_plaintext_entry_is_rejected() {
        let entry = plaintext_entry(b"hello");
        let err = decrypt_entry(&entry, &dek()).unwrap_err();
        assert!(matches!(err, PayloadError::NotEncrypted));
    }

    #[test]
    fn tampered_payload_fails_decrypt() {
        let entry = plaintext_entry(b"sensitive");
        let mut encrypted = encrypt_entry(entry, &dek()).unwrap();
        let last = encrypted.payload.len() - 1;
        encrypted.payload[last] ^= 0x01;
        assert!(matches!(
            decrypt_entry(&encrypted, &dek()),
            Err(PayloadError::Crypto(CryptoError::Aead))
        ));
    }

    #[test]
    fn round_trip_through_outbox_and_applier() {
        // End-to-end zero-knowledge proof: encrypt locally, the entry
        // travels through the outbox queue as ciphertext (here just a
        // direct hand-off), and the receiving side decrypts to recover
        // the plaintext. The "server" in this test never sees the
        // plaintext bytes — it only handles the OutboxEntry struct.
        use crate::outbox::Outbox;
        use crate::sqlite_outbox::SqliteOutbox;
        use aether_db::Pool;

        // Wrap in a tokio runtime since the outbox trait is async.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let pool = Pool::open_in_memory().await.unwrap();
            let outbox = SqliteOutbox::new(pool);

            let plaintext = b"{\"materials.cost\":12.50}".to_vec();
            let entry = OutboxEntry {
                op_id: "op-e2e".into(),
                entity: "materials".into(),
                entity_id: "mat-99".into(),
                op: Op::Update,
                payload: plaintext.clone(),
                hlc_ts: Hlc::new(99, 0, "node-a"),
                parent_hlc: None,
                encrypted: false,
            };
            let sealed = encrypt_entry(entry, &dek()).unwrap();
            outbox.enqueue(sealed).await.unwrap();

            // Poll on the "other side" and decrypt.
            let mut polled = outbox.poll(10).await.unwrap();
            assert_eq!(polled.len(), 1);
            let got = polled.pop().unwrap();
            assert!(got.encrypted, "outbox preserved the encrypted flag");
            assert_ne!(got.payload, plaintext, "outbox stored ciphertext");

            let recovered = decrypt_entry(&got, &dek()).unwrap();
            assert_eq!(recovered, plaintext);
        });
    }
}
