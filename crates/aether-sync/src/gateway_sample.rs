//! `gateway_samples` outbox encoding — bridges
//! [`aether_gateway::BufferedSample`] to the V10
//! `gateway_samples` Supabase table via the existing outbox.
//!
//! ## Why this is separate from `actuator_command`
//! V10's `actuator_command` encoder ledgers per-command audit
//! rows (operator/system actions). V17's `gateway_sample`
//! encoder ledgers per-sample telemetry rows (the gateway's
//! buffered upstream-bound data). Different cardinality, access
//! pattern, retention tier. Forcing them into one table would
//! force every analyst query to filter by `kind`, and the hot-
//! cold rollup story for telemetry (millions of rows/day) is
//! entirely different from the audit story (dozens/min, kept
//! forever).
//!
//! Both encoders share the same `OutboxEntry` plumbing on the
//! client side — encryption via `payload::encrypt_entry` works
//! the same way for both, and `SqliteOutbox::enqueue` is
//! agnostic to `entity`.
//!
//! ## Wire-format invariant
//! The serde JSON for `GatewaySampleRecord` is what lands in the
//! `gateway_samples.payload` column on the server (after
//! envelope unwrap, for encrypted variants — same zero-knowledge
//! tradeoff documented in `payload.rs`). The `entity` and `op_id`
//! fields on the resulting `OutboxEntry` are load-bearing for
//! routing — entity = table name, op_id = stable creation UUID.

use crate::outbox::{Op, OutboxEntry};
use aether_core::Hlc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// The `gateway_samples` table name — used both as the outbox
/// `entity` tag and (on the receiving side) as the Postgres table
/// the row writes to. Const so a typo at the call site becomes
/// a compile error rather than a silent mis-route.
pub const GATEWAY_SAMPLES_ENTITY: &str = "gateway_samples";

/// Client-side mirror of the SQL row, minus `forwarded_at`
/// (server-side default `now()` on insert). The `payload` field
/// is owned bytes here; on the wire it serializes as a JSON
/// array of integers, which the V10 outbox/applier path handles
/// uniformly.
///
/// Validation contract: every field shape mirrors the SQL
/// exactly. Empty `topic` is rejected with `EmptyTopic`; nil
/// UUIDs surface as `NilUuid`; oversized payloads as
/// `PayloadTooLarge`. The CHECK constraints on the server are
/// belt-and-braces — typed client-side validation makes round-
/// trip debugging cheaper.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewaySampleRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub gateway_id: Uuid,
    pub machine_id: Option<Uuid>,
    pub topic: String,
    pub payload: Vec<u8>,
    /// Wall-time at the gateway when it accepted the sample.
    /// Source of truth for upstream HLC ordering.
    pub buffered_at: DateTime<Utc>,
    pub hlc: Hlc,
}

/// Upper bound on payload bytes. Mirrors
/// `aether_gateway::MAX_PAYLOAD_BYTES` (64 KiB). Kept as a local
/// const to avoid a dep from sync onto gateway — both crates
/// settle on the same value via the SQL CHECK constraint, which
/// is the source of truth for the wire.
pub const MAX_SAMPLE_PAYLOAD_BYTES: usize = 65_536;

#[derive(Debug, Error)]
pub enum SampleEncodingError {
    /// `topic` was empty. The SQL CHECK enforces non-empty
    /// `length(topic)` server-side; client-side surfacing avoids
    /// a round-trip plus a CHECK violation that's harder to
    /// debug than a typed error.
    #[error("topic must not be empty")]
    EmptyTopic,
    /// Required UUID was nil. Same rationale as the V10
    /// actuator-command encoder.
    #[error("required UUID field is nil: {0}")]
    NilUuid(&'static str),
    /// Payload bytes exceed the per-sample cap. Larger blobs
    /// (video frames, ML weights) go through the V4 sealed-
    /// frame path, not the gateway telemetry buffer.
    #[error("payload {actual} bytes exceeds cap {cap}")]
    PayloadTooLarge { actual: usize, cap: usize },
    #[error("encoding json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encode a freshly-drained gateway sample as an outbox
/// `Insert`. The op_id matches the record id 1:1 so a stuck
/// outbox entry cross-references with the `gateway_samples` row
/// on the server.
///
/// No `Update` variant — gateway samples are append-only
/// telemetry, not state machines. (`REVOKE UPDATE` on the
/// server enforces this.)
pub fn encode_sample(record: &GatewaySampleRecord) -> Result<OutboxEntry, SampleEncodingError> {
    validate(record)?;
    let payload_bytes = serde_json::to_vec(record)?;
    Ok(OutboxEntry {
        op_id: record.id.to_string(),
        entity: GATEWAY_SAMPLES_ENTITY.into(),
        entity_id: record.id.to_string(),
        op: Op::Insert,
        payload: payload_bytes,
        hlc_ts: record.hlc.clone(),
        parent_hlc: None,
        encrypted: false,
    })
}

fn validate(record: &GatewaySampleRecord) -> Result<(), SampleEncodingError> {
    if record.topic.trim().is_empty() {
        return Err(SampleEncodingError::EmptyTopic);
    }
    if record.id.is_nil() {
        return Err(SampleEncodingError::NilUuid("id"));
    }
    if record.tenant_id.is_nil() {
        return Err(SampleEncodingError::NilUuid("tenant_id"));
    }
    if record.gateway_id.is_nil() {
        return Err(SampleEncodingError::NilUuid("gateway_id"));
    }
    if record.payload.len() > MAX_SAMPLE_PAYLOAD_BYTES {
        return Err(SampleEncodingError::PayloadTooLarge {
            actual: record.payload.len(),
            cap: MAX_SAMPLE_PAYLOAD_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Outbox;
    use crate::payload::{decrypt_entry, encrypt_entry};
    use crate::sqlite_outbox::SqliteOutbox;
    use aether_crypto::derive::SUBKEY_LEN;
    use aether_crypto::kdf::MasterKey;
    use aether_crypto::SubKey;
    use aether_db::Pool;

    fn dek() -> SubKey {
        let master = MasterKey::from_bytes([9u8; SUBKEY_LEN]);
        master.derive_data_dek().unwrap()
    }

    fn fixture_record() -> GatewaySampleRecord {
        GatewaySampleRecord {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            gateway_id: Uuid::new_v4(),
            machine_id: Some(Uuid::new_v4()),
            topic: "factory/line-1/reactor-temp".into(),
            payload: b"{\"value\":72.5}".to_vec(),
            buffered_at: Utc::now(),
            hlc: Hlc::new(100, 0, "gateway-a"),
        }
    }

    #[test]
    fn encode_produces_well_formed_outbox_entry() {
        let rec = fixture_record();
        let entry = encode_sample(&rec).unwrap();
        assert_eq!(entry.entity, GATEWAY_SAMPLES_ENTITY);
        assert_eq!(entry.op, Op::Insert);
        assert_eq!(entry.op_id, rec.id.to_string());
        assert_eq!(entry.entity_id, rec.id.to_string());
        assert_eq!(entry.hlc_ts, rec.hlc);
        assert!(entry.parent_hlc.is_none());
        assert!(!entry.encrypted);
    }

    #[test]
    fn encode_payload_round_trips_to_record() {
        // Wire-format contract: the byte payload deserializes
        // back to the same record — same shape as V10's
        // actuator_command encoder.
        let rec = fixture_record();
        let entry = encode_sample(&rec).unwrap();
        let decoded: GatewaySampleRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn empty_topic_is_rejected_at_encode_time() {
        // The SQL CHECK enforces `length(topic) > 0`; surface
        // client-side too so a typo gets caught before the
        // round-trip.
        let mut rec = fixture_record();
        rec.topic = "".into();
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::EmptyTopic));
    }

    #[test]
    fn whitespace_only_topic_is_rejected_at_encode_time() {
        let mut rec = fixture_record();
        rec.topic = "   ".into();
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::EmptyTopic));
    }

    #[test]
    fn nil_id_is_rejected_with_field_name_in_error() {
        let mut rec = fixture_record();
        rec.id = Uuid::nil();
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::NilUuid("id")));
    }

    #[test]
    fn nil_tenant_id_is_rejected() {
        let mut rec = fixture_record();
        rec.tenant_id = Uuid::nil();
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::NilUuid("tenant_id")));
    }

    #[test]
    fn nil_gateway_id_is_rejected() {
        let mut rec = fixture_record();
        rec.gateway_id = Uuid::nil();
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::NilUuid("gateway_id")));
    }

    #[test]
    fn machine_id_is_optional_and_round_trips_when_none() {
        // Protocol-bridge gateways often have no machine_id —
        // pin that the None case round-trips cleanly.
        let mut rec = fixture_record();
        rec.machine_id = None;
        let entry = encode_sample(&rec).unwrap();
        let decoded: GatewaySampleRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert!(decoded.machine_id.is_none());
    }

    #[test]
    fn payload_at_cap_is_accepted_inclusive() {
        // Boundary: exactly MAX_SAMPLE_PAYLOAD_BYTES is fine.
        let mut rec = fixture_record();
        rec.payload = vec![0u8; MAX_SAMPLE_PAYLOAD_BYTES];
        let entry = encode_sample(&rec).unwrap();
        assert_eq!(entry.entity, GATEWAY_SAMPLES_ENTITY);
    }

    #[test]
    fn payload_over_cap_is_rejected_with_typed_error() {
        let mut rec = fixture_record();
        rec.payload = vec![0u8; MAX_SAMPLE_PAYLOAD_BYTES + 1];
        let err = encode_sample(&rec).unwrap_err();
        assert!(matches!(err, SampleEncodingError::PayloadTooLarge { .. }));
    }

    #[tokio::test]
    async fn end_to_end_outbox_round_trip_preserves_record_through_sqlite() {
        // Full pipeline: encode → SQLite enqueue → SQLite poll →
        // decode. Mirrors the production path on the creating
        // side; the server replays the bytes into Postgres.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();

        let entry = encode_sample(&rec).unwrap();
        outbox.enqueue(entry).await.unwrap();

        let mut polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 1);
        let stored = polled.pop().unwrap();
        let decoded: GatewaySampleRecord = serde_json::from_slice(&stored.payload).unwrap();
        assert_eq!(decoded, rec);
        assert_eq!(stored.entity, GATEWAY_SAMPLES_ENTITY);
        assert_eq!(stored.op, Op::Insert);
    }

    #[tokio::test]
    async fn encrypted_round_trip_preserves_record_and_zero_knowledge() {
        // The V17 zero-knowledge property — same shape as the
        // V10 actuator-command headline test. Payload travels
        // through the outbox as ciphertext; server-routing
        // metadata stays plaintext; decrypt yields a byte-
        // identical record.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();
        let key = dek();

        let plain_entry = encode_sample(&rec).unwrap();
        let plain_payload = plain_entry.payload.clone();

        let sealed = encrypt_entry(plain_entry, &key).unwrap();
        assert!(sealed.encrypted);
        assert_ne!(sealed.payload, plain_payload);
        // Server-routing metadata stays unchanged across the
        // wrap.
        assert_eq!(sealed.entity, GATEWAY_SAMPLES_ENTITY);
        assert_eq!(sealed.op_id, rec.id.to_string());

        outbox.enqueue(sealed).await.unwrap();
        let mut polled = outbox.poll(10).await.unwrap();
        let stored = polled.pop().unwrap();
        assert!(stored.encrypted);
        // Plaintext topic does NOT appear in the stored bytes.
        assert!(!stored
            .payload
            .windows(rec.topic.len())
            .any(|w| w == rec.topic.as_bytes()));

        let recovered_bytes = decrypt_entry(&stored, &key).unwrap();
        let decoded: GatewaySampleRecord = serde_json::from_slice(&recovered_bytes).unwrap();
        assert_eq!(decoded, rec);
    }

    #[tokio::test]
    async fn multiple_samples_each_get_independent_entries_in_outbox() {
        // A WAN-restored drain of N buffered samples produces N
        // independent outbox Inserts. Pin that each is durable
        // independently — a crash between forwards doesn't
        // corrupt the server view.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);

        for i in 0..5 {
            let mut rec = fixture_record();
            rec.payload = vec![i];
            outbox.enqueue(encode_sample(&rec).unwrap()).await.unwrap();
        }

        let polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 5);
        for entry in &polled {
            assert_eq!(entry.entity, GATEWAY_SAMPLES_ENTITY);
            assert_eq!(entry.op, Op::Insert);
        }
        // All entity_ids are distinct (no collision across
        // independent samples).
        let mut ids: Vec<_> = polled.iter().map(|e| e.entity_id.clone()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
