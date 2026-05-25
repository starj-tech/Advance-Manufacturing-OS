//! `EvidenceHashChain` — append-only Merkle-style hash chain
//! for the `compliance_evidence` ledger.
//!
//! ## What this is for
//! The V10/V17/V18/V19 audit tables are tenant-scoped row
//! ledgers with `REVOKE UPDATE, DELETE` for tamper resistance
//! at the database layer. But "database has no UPDATE/DELETE"
//! is a deployment property, not a cryptographic one — a
//! compromised admin or migration accident can still rewrite
//! history.
//!
//! A hash chain layered on top makes tampering DETECTABLE:
//! each evidence entry's hash includes the previous entry's
//! hash, so a single edit invalidates every subsequent hash.
//! Auditors verify the chain by recomputing from a trusted
//! anchor (the first entry's hash, published in an
//! externally-attested transparency log).
//!
//! ## What this is NOT
//! - A blockchain. No proof-of-work, no distributed consensus
//!   — the chain is per-tenant and lives in the tenant's
//!   own audit table. Tamper resistance comes from the
//!   external anchor publication (planned for V31+), not
//!   from a global ledger.
//! - A signature scheme. The chain proves "this sequence
//!   wasn't modified after the fact"; pairing with the V2-era
//!   `aether-crypto::Signer` proves WHO appended each entry.
//!   The two layers compose; this file only owns the chain.
//!
//! ## Hash recipe
//! For each entry i:
//!
//! ```text
//! hash[i] = SHA-256( hash[i-1] || source || canonical_payload[i] )
//! ```
//!
//! with hash[-1] = `GENESIS_HASH` (all zeros). `canonical_
//! payload` is the JSON serialization with sorted object keys
//! so two writers producing the same logical entry get the
//! same hash.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Length of a chain hash in bytes (SHA-256 output).
pub const HASH_LEN: usize = 32;

/// Anchor hash for the first entry in any new chain — all
/// zeros. Externally published as part of the tenant's
/// transparency-log root once the V31+ attestation layer ships.
pub const GENESIS_HASH: [u8; HASH_LEN] = [0u8; HASH_LEN];

#[derive(Debug, Error)]
pub enum ChainError {
    /// Two entries claimed the same `seq` number, or the
    /// chain has a gap in its sequence. Both indicate
    /// tampering or a producer bug.
    #[error("sequence violation at seq {0}")]
    SequenceViolation(u64),
    /// An entry's recorded hash doesn't match what re-hashing
    /// would produce. Concrete tampering signal.
    #[error("hash mismatch at seq {seq}: expected {expected} got {actual}")]
    HashMismatch {
        seq: u64,
        expected: String,
        actual: String,
    },
    /// JSON canonicalization of the payload failed. Almost
    /// never happens (serde_json::to_value of a serde-friendly
    /// type), but surfaced rather than silently dropped.
    #[error("canonicalize: {0}")]
    Canonicalize(String),
}

/// One link in the chain. The recorded hash is computed at
/// append-time and stored alongside the payload. Auditors
/// verify by re-running the recipe.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceLink {
    /// Monotonic per-tenant sequence number. Starts at 0,
    /// increments by 1 per append. Gaps are tamper signals.
    pub seq: u64,
    /// Source slug — which probe / table the entry came from
    /// (e.g. "audit_log", "interlock_events", "actuator_commands").
    /// Persisted into the hash so swapping a row's source
    /// after the fact is detectable.
    pub source: String,
    /// Wall-time at append. Not included in the hash because
    /// the canonical payload should carry its own at-rest
    /// timestamp; the link-level time is for human-readable
    /// audit ordering only.
    pub at: DateTime<Utc>,
    /// Canonical-JSON payload bytes. Same content always
    /// produces the same bytes (sorted keys).
    pub payload: Vec<u8>,
    /// SHA-256 over (prev_hash || source || payload).
    pub hash: [u8; HASH_LEN],
}

impl EvidenceLink {
    /// Hex-string view of the hash. Convenient for log lines
    /// and the externally-published anchor.
    pub fn hash_hex(&self) -> String {
        hex_encode(&self.hash)
    }
}

/// Append-only chain. Construct fresh with `new()`, then
/// `append(source, payload)` for each evidence entry.
pub struct EvidenceHashChain {
    links: Vec<EvidenceLink>,
}

impl EvidenceHashChain {
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Append an entry. Computes the link hash from the
    /// previous tail (or GENESIS_HASH for an empty chain) and
    /// returns the new link.
    pub fn append<T: Serialize>(
        &mut self,
        source: impl Into<String>,
        payload: &T,
    ) -> Result<EvidenceLink, ChainError> {
        let source = source.into();
        let payload_bytes = canonicalize(payload)?;
        let prev_hash = self.tail_hash();
        let hash = compute_hash(&prev_hash, &source, &payload_bytes);
        let link = EvidenceLink {
            seq: self.links.len() as u64,
            source,
            at: Utc::now(),
            payload: payload_bytes,
            hash,
        };
        self.links.push(link.clone());
        Ok(link)
    }

    /// Most recent link's hash, or `GENESIS_HASH` if empty.
    /// The externally-published anchor at a moment in time is
    /// exactly this value.
    pub fn tail_hash(&self) -> [u8; HASH_LEN] {
        self.links.last().map(|l| l.hash).unwrap_or(GENESIS_HASH)
    }

    /// Number of links currently in the chain.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Borrow the in-order links — for serialization to the
    /// `compliance_evidence` table.
    pub fn links(&self) -> &[EvidenceLink] {
        &self.links
    }

    /// Verify the chain end-to-end against the canonical
    /// recipe. Returns Ok if every link's recorded hash
    /// matches what the recipe would produce; returns the
    /// first violation otherwise.
    pub fn verify(links: &[EvidenceLink]) -> Result<(), ChainError> {
        let mut prev_hash = GENESIS_HASH;
        for (idx, link) in links.iter().enumerate() {
            let expected_seq = idx as u64;
            if link.seq != expected_seq {
                return Err(ChainError::SequenceViolation(link.seq));
            }
            let recomputed = compute_hash(&prev_hash, &link.source, &link.payload);
            if recomputed != link.hash {
                return Err(ChainError::HashMismatch {
                    seq: link.seq,
                    expected: hex_encode(&recomputed),
                    actual: hex_encode(&link.hash),
                });
            }
            prev_hash = link.hash;
        }
        Ok(())
    }
}

impl Default for EvidenceHashChain {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_hash(prev: &[u8; HASH_LEN], source: &str, payload: &[u8]) -> [u8; HASH_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(prev);
    hasher.update(source.as_bytes());
    hasher.update(payload);
    let out = hasher.finalize();
    let mut h = [0u8; HASH_LEN];
    h.copy_from_slice(&out);
    h
}

/// Canonical-JSON serialization: sorted object keys so the
/// same logical payload always produces the same bytes
/// regardless of producer's serde field-order. Critical for
/// the chain — two writers building the same audit row from
/// different machines must hash identically.
fn canonicalize<T: Serialize>(value: &T) -> Result<Vec<u8>, ChainError> {
    // serde_json::to_value goes through `Value`, where object
    // keys are stored in a `BTreeMap` (key-sorted). Then
    // serialize that to bytes — emits sorted-key JSON.
    let v = serde_json::to_value(value).map_err(|e| ChainError::Canonicalize(e.to_string()))?;
    serde_json::to_vec(&v).map_err(|e| ChainError::Canonicalize(e.to_string()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_chain_has_genesis_tail() {
        let chain = EvidenceHashChain::new();
        assert_eq!(chain.tail_hash(), GENESIS_HASH);
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn first_append_uses_genesis_as_prev() {
        let mut chain = EvidenceHashChain::new();
        let link = chain.append("audit_log", &json!({"x": 1})).unwrap();
        assert_eq!(link.seq, 0);
        // The link's hash includes GENESIS as prev, so a hand-
        // computed comparison should match.
        let payload = canonicalize(&json!({"x": 1})).unwrap();
        let expected = compute_hash(&GENESIS_HASH, "audit_log", &payload);
        assert_eq!(link.hash, expected);
    }

    #[test]
    fn successive_appends_chain_through_prev_hash() {
        let mut chain = EvidenceHashChain::new();
        let l0 = chain.append("audit_log", &json!({"a": 1})).unwrap();
        let l1 = chain.append("audit_log", &json!({"b": 2})).unwrap();
        // l1's hash MUST include l0's hash as prev. Recompute
        // and verify.
        let payload1 = canonicalize(&json!({"b": 2})).unwrap();
        let expected = compute_hash(&l0.hash, "audit_log", &payload1);
        assert_eq!(l1.hash, expected);
        assert_eq!(l1.seq, 1);
        assert_eq!(chain.tail_hash(), l1.hash);
    }

    #[test]
    fn canonicalize_is_field_order_independent() {
        // Two JSON values with the same fields in different
        // orders MUST produce the same bytes. This is the
        // property that makes the chain replayable across
        // producers.
        let a = canonicalize(&json!({"x": 1, "y": 2})).unwrap();
        let b = canonicalize(&json!({"y": 2, "x": 1})).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn verify_accepts_a_pristine_chain() {
        let mut chain = EvidenceHashChain::new();
        for i in 0..5 {
            chain.append("audit_log", &json!({"i": i})).unwrap();
        }
        EvidenceHashChain::verify(chain.links()).unwrap();
    }

    #[test]
    fn verify_detects_payload_tamper() {
        let mut chain = EvidenceHashChain::new();
        chain.append("audit_log", &json!({"x": 1})).unwrap();
        chain.append("audit_log", &json!({"y": 2})).unwrap();
        chain.append("audit_log", &json!({"z": 3})).unwrap();
        let mut links = chain.links().to_vec();
        // Edit the middle link's payload after the fact.
        links[1].payload = canonicalize(&json!({"y": "EVIL"})).unwrap();
        let err = EvidenceHashChain::verify(&links).unwrap_err();
        match err {
            ChainError::HashMismatch { seq, .. } => assert_eq!(seq, 1),
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn verify_detects_source_tamper() {
        // Swap source on an entry but leave payload alone.
        // Source is also in the hash, so this MUST detect.
        let mut chain = EvidenceHashChain::new();
        chain.append("audit_log", &json!({"x": 1})).unwrap();
        chain.append("audit_log", &json!({"y": 2})).unwrap();
        let mut links = chain.links().to_vec();
        links[1].source = "interlock_events".into();
        let err = EvidenceHashChain::verify(&links).unwrap_err();
        assert!(matches!(err, ChainError::HashMismatch { .. }));
    }

    #[test]
    fn verify_detects_sequence_gap() {
        let mut chain = EvidenceHashChain::new();
        chain.append("audit_log", &json!({"x": 1})).unwrap();
        chain.append("audit_log", &json!({"y": 2})).unwrap();
        chain.append("audit_log", &json!({"z": 3})).unwrap();
        let mut links = chain.links().to_vec();
        // Remove the middle link entirely — seq jumps 0,2.
        // The verifier sees seq=2 at position 1 (expected_seq=1),
        // so the violation surfaces with the OFFENDING seq (2),
        // not the expected one. Pin that contract.
        links.remove(1);
        let err = EvidenceHashChain::verify(&links).unwrap_err();
        assert!(matches!(err, ChainError::SequenceViolation(2)));
    }

    #[test]
    fn verify_detects_hash_tamper_at_tail() {
        // The single most likely tamper attempt: an attacker
        // edits the LATEST hash, hoping nobody notices because
        // there's no downstream to chain it against.
        let mut chain = EvidenceHashChain::new();
        chain.append("audit_log", &json!({"x": 1})).unwrap();
        chain.append("audit_log", &json!({"y": 2})).unwrap();
        let mut links = chain.links().to_vec();
        // Flip a single bit of the tail hash.
        links.last_mut().unwrap().hash[0] ^= 0x01;
        let err = EvidenceHashChain::verify(&links).unwrap_err();
        assert!(matches!(err, ChainError::HashMismatch { .. }));
    }

    #[test]
    fn link_hash_hex_is_64_chars() {
        let mut chain = EvidenceHashChain::new();
        let link = chain.append("audit_log", &json!({})).unwrap();
        let hex = link.hash_hex();
        assert_eq!(hex.len(), 64);
        // No uppercase letters (lowercase convention).
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn tail_hash_returns_genesis_when_empty_and_last_hash_otherwise() {
        let mut chain = EvidenceHashChain::new();
        assert_eq!(chain.tail_hash(), GENESIS_HASH);
        let link = chain.append("audit_log", &json!({"x": 1})).unwrap();
        assert_eq!(chain.tail_hash(), link.hash);
    }

    #[test]
    fn two_chains_with_same_inputs_produce_same_tail_hash() {
        // Determinism property: two tenants building the same
        // ledger from the same inputs end up with the same
        // tail hash. The externally-published anchor can be
        // compared across producers.
        let mut a = EvidenceHashChain::new();
        let mut b = EvidenceHashChain::new();
        for i in 0..4 {
            a.append("audit_log", &json!({"i": i})).unwrap();
            b.append("audit_log", &json!({"i": i})).unwrap();
        }
        assert_eq!(a.tail_hash(), b.tail_hash());
    }

    #[test]
    fn empty_links_verifies_trivially() {
        // Edge case: a fresh tenant has no evidence yet. The
        // verify path must not panic on an empty slice.
        EvidenceHashChain::verify(&[]).unwrap();
    }

    #[test]
    fn evidence_link_serde_round_trips_through_json() {
        // The compliance_evidence table will serialize these
        // rows; pin the round-trip.
        let mut chain = EvidenceHashChain::new();
        let link = chain
            .append("audit_log", &json!({"x": 1, "y": "z"}))
            .unwrap();
        let s = serde_json::to_string(&link).unwrap();
        let back: EvidenceLink = serde_json::from_str(&s).unwrap();
        assert_eq!(back, link);
    }
}
