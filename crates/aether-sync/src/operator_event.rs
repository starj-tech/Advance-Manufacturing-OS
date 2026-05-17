//! `operator_events` outbox encoding — bridges
//! [`aether_hmi::OperatorEvent`] to the V18 `operator_events`
//! Supabase table via the existing outbox.
//!
//! ## Why this is separate from `actuator_command`
//! Same rationale as V17's `gateway_sample` split: different
//! cardinality, access pattern, and retention concerns. Operator
//! events are per-gesture (frequent, fine-grained) while
//! `actuator_commands` is per-action (deliberate, coarse). An
//! analyst's question "how many alarm acknowledgements in the
//! last shift" filters this table; "how many alarms were pushed"
//! filters actuator_commands. Index separately.
//!
//! ## Content-agnostic encoder
//! The encoder takes `event_kind` as a string rather than
//! importing `aether_hmi::OperatorEvent` so the sync crate's
//! dep footprint stays narrow. The caller passes
//! `OperatorEvent::slug()` from `aether-hmi`; the SQL CHECK
//! constraint pins the allowed values.

use crate::outbox::{Op, OutboxEntry};
use aether_core::Hlc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// The `operator_events` table name — outbox entity tag and
/// Postgres table target. Const so typos at the call site are
/// compile errors, not silent mis-routes.
pub const OPERATOR_EVENTS_ENTITY: &str = "operator_events";

/// Client-side mirror of the SQL row, minus `recorded_at`
/// (server-side default `now()`). `payload` carries the
/// structured event details as JSON — empty `{}` for
/// Acknowledge/Cancel, populated for Tap/Swipe/Voice/GazeDwell.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OperatorEventRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub hmi_id: Uuid,
    pub user_id: Option<Uuid>,
    pub machine_id: Option<Uuid>,
    /// Kebab-case event kind — must match the SQL CHECK
    /// constraint (`tap` / `swipe` / `voice` / `gaze-dwell` /
    /// `acknowledge` / `cancel`).
    pub event_kind: String,
    pub payload: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
    pub hlc: Hlc,
}

#[derive(Debug, Error)]
pub enum EventEncodingError {
    /// `event_kind` was empty. The SQL CHECK constraint would
    /// also reject this; client-side surfacing avoids the
    /// round-trip.
    #[error("event_kind must not be empty")]
    EmptyEventKind,
    /// Required UUID was nil. Same rationale as V10 and V17.
    #[error("required UUID field is nil: {0}")]
    NilUuid(&'static str),
    #[error("encoding json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encode a freshly-drained operator event as an outbox
/// `Insert`. The op_id matches the record id 1:1 for cross-
/// reference with the `operator_events` row on the server.
///
/// No `Update` encoder — operator events are append-only audit,
/// not state machines. (`REVOKE UPDATE` on the server enforces
/// this.)
pub fn encode_event(record: &OperatorEventRecord) -> Result<OutboxEntry, EventEncodingError> {
    validate(record)?;
    let payload_bytes = serde_json::to_vec(record)?;
    Ok(OutboxEntry {
        op_id: record.id.to_string(),
        entity: OPERATOR_EVENTS_ENTITY.into(),
        entity_id: record.id.to_string(),
        op: Op::Insert,
        payload: payload_bytes,
        hlc_ts: record.hlc.clone(),
        parent_hlc: None,
        encrypted: false,
    })
}

fn validate(record: &OperatorEventRecord) -> Result<(), EventEncodingError> {
    if record.event_kind.trim().is_empty() {
        return Err(EventEncodingError::EmptyEventKind);
    }
    if record.id.is_nil() {
        return Err(EventEncodingError::NilUuid("id"));
    }
    if record.tenant_id.is_nil() {
        return Err(EventEncodingError::NilUuid("tenant_id"));
    }
    if record.hmi_id.is_nil() {
        return Err(EventEncodingError::NilUuid("hmi_id"));
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
        let master = MasterKey::from_bytes([0xAu8; SUBKEY_LEN]);
        master.derive_data_dek().unwrap()
    }

    fn fixture_record() -> OperatorEventRecord {
        OperatorEventRecord {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            hmi_id: Uuid::new_v4(),
            user_id: Some(Uuid::new_v4()),
            machine_id: Some(Uuid::new_v4()),
            event_kind: "tap".into(),
            payload: serde_json::json!({"region": "alarm-clear"}),
            occurred_at: Utc::now(),
            hlc: Hlc::new(7, 0, "hmi-a"),
        }
    }

    #[test]
    fn encode_produces_well_formed_outbox_entry() {
        let rec = fixture_record();
        let entry = encode_event(&rec).unwrap();
        assert_eq!(entry.entity, OPERATOR_EVENTS_ENTITY);
        assert_eq!(entry.op, Op::Insert);
        assert_eq!(entry.op_id, rec.id.to_string());
        assert_eq!(entry.entity_id, rec.id.to_string());
        assert_eq!(entry.hlc_ts, rec.hlc);
        assert!(entry.parent_hlc.is_none());
        assert!(!entry.encrypted);
    }

    #[test]
    fn encode_payload_round_trips_to_record() {
        let rec = fixture_record();
        let entry = encode_event(&rec).unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn empty_event_kind_is_rejected_at_encode_time() {
        let mut rec = fixture_record();
        rec.event_kind = "".into();
        let err = encode_event(&rec).unwrap_err();
        assert!(matches!(err, EventEncodingError::EmptyEventKind));
    }

    #[test]
    fn whitespace_only_event_kind_is_rejected_at_encode_time() {
        let mut rec = fixture_record();
        rec.event_kind = "   ".into();
        let err = encode_event(&rec).unwrap_err();
        assert!(matches!(err, EventEncodingError::EmptyEventKind));
    }

    #[test]
    fn nil_id_is_rejected_with_field_name_in_error() {
        let mut rec = fixture_record();
        rec.id = Uuid::nil();
        let err = encode_event(&rec).unwrap_err();
        assert!(matches!(err, EventEncodingError::NilUuid("id")));
    }

    #[test]
    fn nil_tenant_id_is_rejected() {
        let mut rec = fixture_record();
        rec.tenant_id = Uuid::nil();
        let err = encode_event(&rec).unwrap_err();
        assert!(matches!(err, EventEncodingError::NilUuid("tenant_id")));
    }

    #[test]
    fn nil_hmi_id_is_rejected() {
        let mut rec = fixture_record();
        rec.hmi_id = Uuid::nil();
        let err = encode_event(&rec).unwrap_err();
        assert!(matches!(err, EventEncodingError::NilUuid("hmi_id")));
    }

    #[test]
    fn user_id_is_optional_for_unauthenticated_pendant_sessions() {
        // Pendant shared surfaces don't bind a user. Pin that
        // None round-trips cleanly.
        let mut rec = fixture_record();
        rec.user_id = None;
        let entry = encode_event(&rec).unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert!(decoded.user_id.is_none());
    }

    #[test]
    fn machine_id_is_optional_for_system_wide_gestures() {
        // Home-dashboard alarm acknowledgement isn't tied to a
        // specific machine. Pin that None round-trips.
        let mut rec = fixture_record();
        rec.machine_id = None;
        let entry = encode_event(&rec).unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert!(decoded.machine_id.is_none());
    }

    #[test]
    fn acknowledge_event_round_trips_with_empty_payload() {
        // Acknowledge/Cancel events carry no structured payload
        // beyond the kind. Pin that the empty `{}` JSON is the
        // canonical representation and survives serde.
        let mut rec = fixture_record();
        rec.event_kind = "acknowledge".into();
        rec.payload = serde_json::json!({});
        let entry = encode_event(&rec).unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded.event_kind, "acknowledge");
        assert_eq!(decoded.payload, serde_json::json!({}));
    }

    #[tokio::test]
    async fn end_to_end_outbox_round_trip_preserves_record_through_sqlite() {
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();

        let entry = encode_event(&rec).unwrap();
        outbox.enqueue(entry).await.unwrap();

        let mut polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 1);
        let stored = polled.pop().unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&stored.payload).unwrap();
        assert_eq!(decoded, rec);
        assert_eq!(stored.entity, OPERATOR_EVENTS_ENTITY);
        assert_eq!(stored.op, Op::Insert);
    }

    #[tokio::test]
    async fn encrypted_round_trip_preserves_record_and_zero_knowledge() {
        // Voice utterances are sensitive — they may carry
        // operator names, recipe codes, free-form speech.
        // Pin that the utterance text does NOT appear in the
        // sealed bytes.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let mut rec = fixture_record();
        rec.event_kind = "voice".into();
        rec.payload = serde_json::json!({
            "utterance": "halt-recipe-7-alpha",
            "confidence": 0.92,
        });
        let key = dek();

        let plain_entry = encode_event(&rec).unwrap();
        let plain_payload = plain_entry.payload.clone();

        let sealed = encrypt_entry(plain_entry, &key).unwrap();
        assert!(sealed.encrypted);
        assert_ne!(sealed.payload, plain_payload);
        assert_eq!(sealed.entity, OPERATOR_EVENTS_ENTITY);

        outbox.enqueue(sealed).await.unwrap();
        let mut polled = outbox.poll(10).await.unwrap();
        let stored = polled.pop().unwrap();
        assert!(stored.encrypted);
        // Voice utterance text MUST NOT leak through the
        // outbox storage.
        let marker = b"halt-recipe-7-alpha";
        assert!(!stored.payload.windows(marker.len()).any(|w| w == marker));

        let recovered_bytes = decrypt_entry(&stored, &key).unwrap();
        let decoded: OperatorEventRecord = serde_json::from_slice(&recovered_bytes).unwrap();
        assert_eq!(decoded, rec);
    }

    #[tokio::test]
    async fn bulk_drain_produces_independent_outbox_entries() {
        // A `pending_events` drain returns N events; each gets
        // an independent outbox Insert with a distinct id.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);

        for kind in ["tap", "swipe", "acknowledge"] {
            let mut rec = fixture_record();
            rec.event_kind = kind.into();
            outbox.enqueue(encode_event(&rec).unwrap()).await.unwrap();
        }

        let polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 3);
        for entry in &polled {
            assert_eq!(entry.entity, OPERATOR_EVENTS_ENTITY);
            assert_eq!(entry.op, Op::Insert);
        }
        let mut ids: Vec<_> = polled.iter().map(|e| e.entity_id.clone()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
