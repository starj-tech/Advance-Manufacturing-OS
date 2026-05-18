//! `scan_events` outbox encoding — bridges
//! [`aether_autoid::ScannedPayload`] (+ optional V12
//! `HandlerOutcome`) to the V19 `scan_events` Supabase table via
//! the existing outbox.
//!
//! ## Why this is separate from `actuator_command`
//! Same rationale as V17 (`gateway_sample`) and V18
//! (`operator_event`): different domain, different access
//! pattern, different retention. `actuator_commands` audits the
//! Scan dispatch row (permit, outcome metadata); this table
//! ledgers the decoded data + symbology class + optional
//! handler verdict. An FSMA 204 lot-traceability query
//! ("every GS1-128 scanned in the last hour") filters this
//! table; the operator-action audit query filters
//! actuator_commands. Splitting them keeps each cheap on its
//! own index.
//!
//! ## Content-agnostic encoder
//! The encoder takes `scan_class` and `handler_action` as
//! strings rather than importing from `aether-autoid` so the
//! sync crate's dep footprint stays narrow. Callers pass
//! `ScanClass::slug()` and `HandlerAction::slug()`; the SQL
//! CHECK constraints pin the allowed values.

use crate::outbox::{Op, OutboxEntry};
use aether_core::Hlc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const SCAN_EVENTS_ENTITY: &str = "scan_events";

/// Upper bound on payload bytes. Mirrors the SQL CHECK on the
/// server (`octet_length(payload) <= 4096`); 4 KiB is 20x the
/// largest scan payload observed in pilots (multi-AI GS1-128 lot
/// string). Larger NDEF dumps take the V4 sealed-frame path.
pub const MAX_SCAN_PAYLOAD_BYTES: usize = 4096;

/// Client-side mirror of the SQL row, minus `recorded_at`
/// (server default `now()`). All handler-related fields are
/// `Option` so a raw scan with no pipeline routing encodes
/// cleanly.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScanEventRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub scanner_id: Uuid,
    pub machine_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    /// Kebab-case symbology / RF slug from
    /// `aether_autoid::ScanClass::slug()`. Must match the SQL
    /// CHECK constraint enumeration.
    pub scan_class: String,
    /// Raw decoded bytes. UTF-8 text for 1D/2D classes, binary
    /// for RFID/NFC. Capped at `MAX_SCAN_PAYLOAD_BYTES`.
    pub payload: Vec<u8>,
    /// Vendor confidence 0..=1. None for readers that don't
    /// expose one (RFID/NFC).
    pub confidence: Option<f32>,
    /// V12 ScanPipeline handler name. None for unrouted scans.
    pub handler_name: Option<String>,
    /// Kebab-case `HandlerAction` slug
    /// (acknowledge / reject / trigger). None for unrouted
    /// scans.
    pub handler_action: Option<String>,
    pub handler_summary: Option<String>,
    pub scanned_at: DateTime<Utc>,
    pub hlc: Hlc,
}

#[derive(Debug, Error)]
pub enum ScanEncodingError {
    /// `scan_class` was empty. Server CHECK rejects empty +
    /// non-enum values; client-side surfaces the empty case
    /// loudly to avoid the round-trip.
    #[error("scan_class must not be empty")]
    EmptyScanClass,
    /// Required UUID was nil — same rationale as the V10/V17/V18
    /// encoders.
    #[error("required UUID field is nil: {0}")]
    NilUuid(&'static str),
    /// `confidence` outside 0..=1. Server CHECK belt-and-braces
    /// the same constraint; client-side typed error is easier
    /// to debug than a CHECK violation message.
    #[error("confidence {value} outside 0..=1")]
    InvalidConfidence { value: f32 },
    /// Payload exceeds `MAX_SCAN_PAYLOAD_BYTES`. Larger blobs
    /// take the V4 sealed-frame path, not the scan ledger.
    #[error("payload {actual} bytes exceeds cap {cap}")]
    PayloadTooLarge { actual: usize, cap: usize },
    /// `handler_action` was set but isn't one of the V12 slugs.
    /// Surface client-side rather than hitting the SQL CHECK.
    #[error("handler_action {value:?} must be acknowledge/reject/trigger")]
    InvalidHandlerAction { value: String },
    #[error("encoding json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encode a freshly-drained scan event as an outbox `Insert`.
/// No `Update` encoder — scan events are append-only audit,
/// matching the server-side REVOKE UPDATE.
pub fn encode_scan(record: &ScanEventRecord) -> Result<OutboxEntry, ScanEncodingError> {
    validate(record)?;
    let payload_bytes = serde_json::to_vec(record)?;
    Ok(OutboxEntry {
        op_id: record.id.to_string(),
        entity: SCAN_EVENTS_ENTITY.into(),
        entity_id: record.id.to_string(),
        op: Op::Insert,
        payload: payload_bytes,
        hlc_ts: record.hlc.clone(),
        parent_hlc: None,
        encrypted: false,
    })
}

fn validate(record: &ScanEventRecord) -> Result<(), ScanEncodingError> {
    if record.scan_class.trim().is_empty() {
        return Err(ScanEncodingError::EmptyScanClass);
    }
    if record.id.is_nil() {
        return Err(ScanEncodingError::NilUuid("id"));
    }
    if record.tenant_id.is_nil() {
        return Err(ScanEncodingError::NilUuid("tenant_id"));
    }
    if record.scanner_id.is_nil() {
        return Err(ScanEncodingError::NilUuid("scanner_id"));
    }
    if let Some(c) = record.confidence {
        if !(0.0..=1.0).contains(&c) || c.is_nan() {
            return Err(ScanEncodingError::InvalidConfidence { value: c });
        }
    }
    if record.payload.len() > MAX_SCAN_PAYLOAD_BYTES {
        return Err(ScanEncodingError::PayloadTooLarge {
            actual: record.payload.len(),
            cap: MAX_SCAN_PAYLOAD_BYTES,
        });
    }
    if let Some(action) = &record.handler_action {
        if !matches!(action.as_str(), "acknowledge" | "reject" | "trigger") {
            return Err(ScanEncodingError::InvalidHandlerAction {
                value: action.clone(),
            });
        }
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
        let master = MasterKey::from_bytes([0xBu8; SUBKEY_LEN]);
        master.derive_data_dek().unwrap()
    }

    fn fixture_record() -> ScanEventRecord {
        ScanEventRecord {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            scanner_id: Uuid::new_v4(),
            machine_id: Some(Uuid::new_v4()),
            user_id: Some(Uuid::new_v4()),
            scan_class: "gs1-128".into(),
            payload: b"01070112345678901721".to_vec(),
            confidence: Some(0.95),
            handler_name: Some("lot-consumption".into()),
            handler_action: Some("trigger".into()),
            handler_summary: Some("lot recorded".into()),
            scanned_at: Utc::now(),
            hlc: Hlc::new(11, 0, "scanner-a"),
        }
    }

    #[test]
    fn encode_produces_well_formed_outbox_entry() {
        let rec = fixture_record();
        let entry = encode_scan(&rec).unwrap();
        assert_eq!(entry.entity, SCAN_EVENTS_ENTITY);
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
        let entry = encode_scan(&rec).unwrap();
        let decoded: ScanEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn empty_scan_class_is_rejected_at_encode_time() {
        let mut rec = fixture_record();
        rec.scan_class = "".into();
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::EmptyScanClass));
    }

    #[test]
    fn nil_id_is_rejected_with_field_name_in_error() {
        let mut rec = fixture_record();
        rec.id = Uuid::nil();
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::NilUuid("id")));
    }

    #[test]
    fn nil_tenant_id_is_rejected() {
        let mut rec = fixture_record();
        rec.tenant_id = Uuid::nil();
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::NilUuid("tenant_id")));
    }

    #[test]
    fn nil_scanner_id_is_rejected() {
        let mut rec = fixture_record();
        rec.scanner_id = Uuid::nil();
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::NilUuid("scanner_id")));
    }

    #[test]
    fn confidence_below_zero_is_rejected() {
        let mut rec = fixture_record();
        rec.confidence = Some(-0.1);
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::InvalidConfidence { .. }));
    }

    #[test]
    fn confidence_above_one_is_rejected() {
        let mut rec = fixture_record();
        rec.confidence = Some(1.5);
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::InvalidConfidence { .. }));
    }

    #[test]
    fn nan_confidence_is_rejected() {
        // NaN is technically not "outside 0..=1" by partial-ord
        // semantics, but it's never a valid reader confidence —
        // surface explicitly so a downstream filter on
        // `confidence >= 0.9` doesn't silently exclude all rows.
        let mut rec = fixture_record();
        rec.confidence = Some(f32::NAN);
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::InvalidConfidence { .. }));
    }

    #[test]
    fn boundary_confidence_values_zero_and_one_are_accepted_inclusive() {
        for c in [Some(0.0), Some(1.0), None] {
            let mut rec = fixture_record();
            rec.confidence = c;
            encode_scan(&rec).expect("boundary confidence should encode");
        }
    }

    #[test]
    fn payload_at_cap_is_accepted_inclusive() {
        let mut rec = fixture_record();
        rec.payload = vec![0u8; MAX_SCAN_PAYLOAD_BYTES];
        let entry = encode_scan(&rec).unwrap();
        assert_eq!(entry.entity, SCAN_EVENTS_ENTITY);
    }

    #[test]
    fn payload_over_cap_is_rejected_with_typed_error() {
        let mut rec = fixture_record();
        rec.payload = vec![0u8; MAX_SCAN_PAYLOAD_BYTES + 1];
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(err, ScanEncodingError::PayloadTooLarge { .. }));
    }

    #[test]
    fn invalid_handler_action_is_rejected() {
        let mut rec = fixture_record();
        rec.handler_action = Some("approve".into()); // not in V12 set
        let err = encode_scan(&rec).unwrap_err();
        assert!(matches!(
            err,
            ScanEncodingError::InvalidHandlerAction { .. }
        ));
    }

    #[test]
    fn unrouted_scan_with_no_handler_metadata_encodes_cleanly() {
        // Raw scans (no pipeline) have all handler_* fields
        // None. Pin that the None case round-trips.
        let mut rec = fixture_record();
        rec.handler_name = None;
        rec.handler_action = None;
        rec.handler_summary = None;
        let entry = encode_scan(&rec).unwrap();
        let decoded: ScanEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert!(decoded.handler_name.is_none());
        assert!(decoded.handler_action.is_none());
        assert!(decoded.handler_summary.is_none());
    }

    #[test]
    fn rfid_scan_with_binary_payload_and_no_confidence_encodes_cleanly() {
        // RFID UIDs are binary, readers don't expose confidence
        // — pin both. Round-trip preserves bytes verbatim.
        let mut rec = fixture_record();
        rec.scan_class = "rfid-iso-15693".into();
        rec.payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        rec.confidence = None;
        let entry = encode_scan(&rec).unwrap();
        let decoded: ScanEventRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(decoded.confidence.is_none());
    }

    #[tokio::test]
    async fn end_to_end_outbox_round_trip_preserves_record_through_sqlite() {
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();

        let entry = encode_scan(&rec).unwrap();
        outbox.enqueue(entry).await.unwrap();

        let mut polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 1);
        let stored = polled.pop().unwrap();
        let decoded: ScanEventRecord = serde_json::from_slice(&stored.payload).unwrap();
        assert_eq!(decoded, rec);
        assert_eq!(stored.entity, SCAN_EVENTS_ENTITY);
        assert_eq!(stored.op, Op::Insert);
    }

    #[tokio::test]
    async fn encrypted_round_trip_preserves_record_and_zero_knowledge() {
        // Scan payloads can carry sensitive data — operator
        // badge UIDs, recipe codes, lot identifiers that the
        // factory considers trade secret. Pin that the
        // payload bytes do NOT appear in the sealed outbox row.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let mut rec = fixture_record();
        // Sensitive marker — a recognizable lot code that we'll
        // grep for in the ciphertext to confirm zero-knowledge.
        let marker = b"SECRET-LOT-2026-ALPHA";
        rec.payload = marker.to_vec();
        let key = dek();

        let plain_entry = encode_scan(&rec).unwrap();
        let sealed = encrypt_entry(plain_entry, &key).unwrap();
        assert!(sealed.encrypted);
        // Server-routing metadata stays plaintext (entity +
        // op_id needed for routing).
        assert_eq!(sealed.entity, SCAN_EVENTS_ENTITY);

        outbox.enqueue(sealed).await.unwrap();
        let mut polled = outbox.poll(10).await.unwrap();
        let stored = polled.pop().unwrap();
        assert!(stored.encrypted);
        // The sensitive lot code MUST NOT appear in the stored
        // ciphertext bytes.
        assert!(!stored.payload.windows(marker.len()).any(|w| w == marker));

        let recovered_bytes = decrypt_entry(&stored, &key).unwrap();
        let decoded: ScanEventRecord = serde_json::from_slice(&recovered_bytes).unwrap();
        assert_eq!(decoded, rec);
    }

    #[tokio::test]
    async fn bulk_drain_produces_independent_outbox_entries() {
        // A continuous-mode RFID antenna drain yields N
        // independent samples; each gets its own outbox row
        // with a distinct id.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);

        for i in 0..4 {
            let mut rec = fixture_record();
            rec.payload = vec![i];
            outbox.enqueue(encode_scan(&rec).unwrap()).await.unwrap();
        }

        let polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 4);
        for entry in &polled {
            assert_eq!(entry.entity, SCAN_EVENTS_ENTITY);
            assert_eq!(entry.op, Op::Insert);
        }
        let mut ids: Vec<_> = polled.iter().map(|e| e.entity_id.clone()).collect();
        ids.sort();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }
}
