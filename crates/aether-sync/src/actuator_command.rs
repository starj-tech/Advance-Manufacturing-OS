//! `actuator_commands` outbox encoding — bridges the
//! V1 `aether-actuators` dispatch path to the V10 `actuator_commands`
//! Supabase table via the existing outbox.
//!
//! ## Why this lives in `aether-sync`
//! Two crates already converge here: `aether-crypto` (envelope
//! sealing in `payload.rs`) and `aether-db` (SQLite outbox queue).
//! The actuator-command record needs both — sealed payloads for
//! sensitive frames, durable enqueue for offline-first operation —
//! so the encoding helper sits with the other sync-side bridges.
//!
//! The helper is *content-agnostic* in the sense that the kind tag
//! is passed in as a string rather than imported from
//! `aether-actuators`. That keeps the sync crate's dependency
//! footprint unchanged and lets the same encoder serve future
//! actuator categories (PLC writes, HMI gestures) without revisiting
//! this layer.
//!
//! ## Wire-format invariant
//! The serde JSON for `ActuatorCommandRecord` is what lands inside
//! the `actuator_commands.payload` column on the server (after
//! envelope unwrap, for encrypted variants). The `entity` and
//! `op_id` fields on the resulting `OutboxEntry` are load-bearing
//! for routing — same conventions as every other replicated entity
//! (entity = table name, op_id = stable creation UUID).

use crate::outbox::{Op, OutboxEntry};
use aether_core::Hlc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// The `actuator_commands` table name — used both as the outbox
/// `entity` tag and (on the receiving side) as the Postgres table
/// the row writes to. Pinned as a const so a typo at the call site
/// becomes a compile error rather than a silent mis-route.
pub const ACTUATOR_COMMANDS_ENTITY: &str = "actuator_commands";

/// Status lifecycle. Matches the `CHECK` constraint on
/// `actuator_commands.status` exactly — adding a variant here is
/// a schema migration on the server side, and vice versa.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActuatorCommandStatus {
    Pending,
    InFlight,
    Completed,
    Failed,
    Preempted,
}

impl ActuatorCommandStatus {
    /// Kebab-case slug that matches the Postgres CHECK constraint.
    pub fn slug(&self) -> &'static str {
        match self {
            ActuatorCommandStatus::Pending => "pending",
            ActuatorCommandStatus::InFlight => "in-flight",
            ActuatorCommandStatus::Completed => "completed",
            ActuatorCommandStatus::Failed => "failed",
            ActuatorCommandStatus::Preempted => "preempted",
        }
    }

    /// Terminal states never transition further. The server-side RLS
    /// `scoped_update_actuator_commands` enforces this; clients use
    /// this method to refuse the bad write before it leaves the
    /// outbox and wastes a round-trip.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ActuatorCommandStatus::Completed
                | ActuatorCommandStatus::Failed
                | ActuatorCommandStatus::Preempted
        )
    }
}

/// Mirror of the `actuator_commands` row, minus the columns the DB
/// fills in (`created_at` defaults to `now()` on insert when not
/// provided; we pass it through explicitly so offline-created rows
/// preserve their local wall time).
///
/// Validation contract: every field shape mirrors the SQL exactly.
/// String fields can't be empty where the SQL has NOT NULL —
/// constructors enforce this, see `EncodingError::EmptyCommandKind`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActuatorCommandRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actuator_id: Uuid,
    pub machine_id: Option<Uuid>,
    pub issued_by: Option<Uuid>,
    /// Kebab-case CommandKind slug. Caller passes
    /// `ActuatorCommand::kind().slug()` from `aether-actuators` —
    /// pinned by the CHECK constraint on the SQL side.
    pub command_kind: String,
    /// Command arguments as JSON. The full `ActuatorCommand` serde
    /// shape is the natural payload, but the encoder doesn't enforce
    /// that — any JSON value is fine for forward-compat.
    pub payload: serde_json::Value,
    pub status: ActuatorCommandStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// UUID of the `ActuatorPermit` that was consumed to dispatch
    /// this command. Captured for forensic linkage with
    /// `interlock_events`; the permit itself isn't persisted because
    /// it's single-use + TTL-bound and has no value after consumption.
    pub permit_id: Option<Uuid>,
    /// Dispatch result body for completed rows (frame UUID, end
    /// pose, route status, …). NULL for non-terminal states.
    pub result: Option<serde_json::Value>,
    pub error_class: Option<String>,
    pub error_detail: Option<String>,
    pub estop_tripped_at: Option<DateTime<Utc>>,
    pub hlc: Hlc,
}

#[derive(Debug, Error)]
pub enum EncodingError {
    /// `command_kind` was empty. The SQL CHECK constraint would
    /// also reject this, but catching it client-side avoids a
    /// round-trip + a CHECK violation that's harder to debug than
    /// a typed error.
    #[error("command_kind must not be empty")]
    EmptyCommandKind,
    /// `tenant_id`, `actuator_id`, or `id` was the nil UUID. Nil
    /// UUIDs would pass FK validation (no FK on actuator_id /
    /// tenant_id check beyond NOT NULL) but represent a bug at the
    /// caller — `Uuid::nil()` is the moral equivalent of None and
    /// shouldn't reach the outbox.
    #[error("required UUID field is nil: {0}")]
    NilUuid(&'static str),
    /// JSON serialization of the record failed. Practically
    /// impossible (all fields are owned + serde-friendly) but
    /// surfaced explicitly rather than swallowed.
    #[error("encoding json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encode a freshly-created actuator-command row as an outbox
/// `Insert`. The op_id matches the record id 1:1 so a stuck
/// outbox entry can be cross-referenced by id with the
/// `actuator_commands` row on the server.
///
/// Subsequent state transitions encode as `Op::Update` via
/// [`encode_status_transition`] with the same `id`/`op_id`. The
/// outbox replay path on the server applies them in HLC order, so
/// even out-of-order pull doesn't risk a `pending` overwriting a
/// later `completed`.
pub fn encode_command_creation(
    record: &ActuatorCommandRecord,
) -> Result<OutboxEntry, EncodingError> {
    validate(record)?;
    let payload_bytes = serde_json::to_vec(record)?;
    Ok(OutboxEntry {
        op_id: record.id.to_string(),
        entity: ACTUATOR_COMMANDS_ENTITY.into(),
        entity_id: record.id.to_string(),
        op: Op::Insert,
        payload: payload_bytes,
        hlc_ts: record.hlc.clone(),
        parent_hlc: None,
        encrypted: false,
    })
}

/// Encode a status transition (in_flight, completed, failed,
/// preempted) for an existing record. The full record is re-
/// serialized so the server-side applier doesn't need to track
/// deltas — bytes in, row out — and the partial-write atomicity
/// is preserved across HLC re-ordering.
///
/// `parent_hlc` carries the HLC of the previous version of the row
/// so the server can detect divergent histories (two clients each
/// applied a transition from the same parent). The HLC-LWW
/// reconciler in `aether-sync::reconcile` handles the conflict.
pub fn encode_status_transition(
    record: &ActuatorCommandRecord,
    parent_hlc: Hlc,
) -> Result<OutboxEntry, EncodingError> {
    validate(record)?;
    let payload_bytes = serde_json::to_vec(record)?;
    Ok(OutboxEntry {
        op_id: format!("{}@{}", record.id, record.hlc),
        entity: ACTUATOR_COMMANDS_ENTITY.into(),
        entity_id: record.id.to_string(),
        op: Op::Update,
        payload: payload_bytes,
        hlc_ts: record.hlc.clone(),
        parent_hlc: Some(parent_hlc),
        encrypted: false,
    })
}

fn validate(record: &ActuatorCommandRecord) -> Result<(), EncodingError> {
    if record.command_kind.trim().is_empty() {
        return Err(EncodingError::EmptyCommandKind);
    }
    if record.id.is_nil() {
        return Err(EncodingError::NilUuid("id"));
    }
    if record.tenant_id.is_nil() {
        return Err(EncodingError::NilUuid("tenant_id"));
    }
    if record.actuator_id.is_nil() {
        return Err(EncodingError::NilUuid("actuator_id"));
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
        // Same shape as payload.rs tests — derive from a fixed
        // master so the test is deterministic.
        let master = MasterKey::from_bytes([7u8; SUBKEY_LEN]);
        master.derive_data_dek().unwrap()
    }

    fn fixture_record() -> ActuatorCommandRecord {
        ActuatorCommandRecord {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            actuator_id: Uuid::new_v4(),
            machine_id: Some(Uuid::new_v4()),
            issued_by: Some(Uuid::new_v4()),
            command_kind: "move-joint".into(),
            payload: serde_json::json!({
                "kind": "move-joint", "joint": 2, "target_rad": 1.57
            }),
            status: ActuatorCommandStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            permit_id: Some(Uuid::new_v4()),
            result: None,
            error_class: None,
            error_detail: None,
            estop_tripped_at: None,
            hlc: Hlc::new(42, 0, "node-a"),
        }
    }

    #[test]
    fn status_slug_matches_sql_check_constraint() {
        // The SQL CHECK constraint hard-codes the exact strings.
        // Pin them here so a rename in code immediately fails this
        // test rather than the migration deploy.
        assert_eq!(ActuatorCommandStatus::Pending.slug(), "pending");
        assert_eq!(ActuatorCommandStatus::InFlight.slug(), "in-flight");
        assert_eq!(ActuatorCommandStatus::Completed.slug(), "completed");
        assert_eq!(ActuatorCommandStatus::Failed.slug(), "failed");
        assert_eq!(ActuatorCommandStatus::Preempted.slug(), "preempted");
    }

    #[test]
    fn is_terminal_distinguishes_lifecycle_states() {
        assert!(!ActuatorCommandStatus::Pending.is_terminal());
        assert!(!ActuatorCommandStatus::InFlight.is_terminal());
        assert!(ActuatorCommandStatus::Completed.is_terminal());
        assert!(ActuatorCommandStatus::Failed.is_terminal());
        assert!(ActuatorCommandStatus::Preempted.is_terminal());
    }

    #[test]
    fn status_serde_uses_kebab_case_on_wire() {
        // The Postgres CHECK constraint is kebab-case. The JSON
        // round-trip MUST be kebab-case to match.
        let s = serde_json::to_string(&ActuatorCommandStatus::InFlight).unwrap();
        assert_eq!(s, "\"in-flight\"");
        let back: ActuatorCommandStatus = serde_json::from_str("\"preempted\"").unwrap();
        assert_eq!(back, ActuatorCommandStatus::Preempted);
    }

    #[test]
    fn encode_creation_produces_well_formed_outbox_entry() {
        let rec = fixture_record();
        let entry = encode_command_creation(&rec).unwrap();
        assert_eq!(entry.entity, ACTUATOR_COMMANDS_ENTITY);
        assert_eq!(entry.op, Op::Insert);
        assert_eq!(entry.op_id, rec.id.to_string());
        assert_eq!(entry.entity_id, rec.id.to_string());
        assert_eq!(entry.hlc_ts, rec.hlc);
        assert!(entry.parent_hlc.is_none());
        assert!(!entry.encrypted);
    }

    #[test]
    fn encode_creation_payload_round_trips_to_record() {
        // The byte payload must deserialize back to the same record
        // — that's the wire-format contract for the server applier.
        let rec = fixture_record();
        let entry = encode_command_creation(&rec).unwrap();
        let decoded: ActuatorCommandRecord = serde_json::from_slice(&entry.payload).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn encode_transition_uses_update_op_and_carries_parent_hlc() {
        let mut rec = fixture_record();
        let parent = rec.hlc.clone();
        rec.status = ActuatorCommandStatus::InFlight;
        rec.hlc = Hlc::new(43, 0, "node-a");
        rec.started_at = Some(Utc::now());

        let entry = encode_status_transition(&rec, parent.clone()).unwrap();
        assert_eq!(entry.op, Op::Update);
        assert_eq!(entry.entity_id, rec.id.to_string());
        assert_eq!(entry.hlc_ts, rec.hlc);
        assert_eq!(entry.parent_hlc, Some(parent));
        // Transition op_ids include the HLC so they're distinct
        // from the creation op_id and from each other.
        assert!(entry.op_id.contains(&rec.id.to_string()));
        assert!(entry.op_id.contains('@'));
    }

    #[test]
    fn encode_transition_distinguishes_multiple_updates() {
        // Two successive transitions on the same record must
        // produce distinct op_ids — otherwise the outbox's idempotency
        // dedupe would collapse them.
        let rec = fixture_record();
        let mut r1 = rec.clone();
        r1.status = ActuatorCommandStatus::InFlight;
        r1.hlc = Hlc::new(43, 0, "node-a");

        let mut r2 = r1.clone();
        r2.status = ActuatorCommandStatus::Completed;
        r2.hlc = Hlc::new(44, 0, "node-a");

        let e1 = encode_status_transition(&r1, rec.hlc.clone()).unwrap();
        let e2 = encode_status_transition(&r2, r1.hlc.clone()).unwrap();
        assert_ne!(e1.op_id, e2.op_id);
    }

    #[test]
    fn empty_command_kind_is_rejected_at_encode_time() {
        // Catch the CHECK violation client-side rather than at the
        // server round-trip.
        let mut rec = fixture_record();
        rec.command_kind = "".into();
        let err = encode_command_creation(&rec).unwrap_err();
        assert!(matches!(err, EncodingError::EmptyCommandKind));
    }

    #[test]
    fn whitespace_only_command_kind_is_rejected_at_encode_time() {
        // The CHECK constraint enforces a specific enum, so a
        // " " value would also fail — surface it as a typed error.
        let mut rec = fixture_record();
        rec.command_kind = "   ".into();
        let err = encode_command_creation(&rec).unwrap_err();
        assert!(matches!(err, EncodingError::EmptyCommandKind));
    }

    #[test]
    fn nil_id_is_rejected_with_field_name_in_error() {
        let mut rec = fixture_record();
        rec.id = Uuid::nil();
        let err = encode_command_creation(&rec).unwrap_err();
        assert!(matches!(err, EncodingError::NilUuid("id")), "got {err:?}");
    }

    #[test]
    fn nil_tenant_id_is_rejected() {
        let mut rec = fixture_record();
        rec.tenant_id = Uuid::nil();
        let err = encode_command_creation(&rec).unwrap_err();
        assert!(matches!(err, EncodingError::NilUuid("tenant_id")));
    }

    #[test]
    fn nil_actuator_id_is_rejected() {
        let mut rec = fixture_record();
        rec.actuator_id = Uuid::nil();
        let err = encode_command_creation(&rec).unwrap_err();
        assert!(matches!(err, EncodingError::NilUuid("actuator_id")));
    }

    #[tokio::test]
    async fn end_to_end_outbox_round_trip_preserves_record_through_sqlite() {
        // Full pipeline: encode → SQLite enqueue → SQLite poll →
        // decode. Mirrors the production path on the *creating*
        // side; the receiving side replays the same bytes into
        // Postgres.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();

        let entry = encode_command_creation(&rec).unwrap();
        outbox.enqueue(entry).await.unwrap();

        let mut polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 1);
        let stored = polled.pop().unwrap();
        let decoded: ActuatorCommandRecord = serde_json::from_slice(&stored.payload).unwrap();
        assert_eq!(decoded, rec);
        // The outbox tagging survived the round-trip too.
        assert_eq!(stored.entity, ACTUATOR_COMMANDS_ENTITY);
        assert_eq!(stored.op, Op::Insert);
    }

    #[tokio::test]
    async fn encrypted_round_trip_preserves_record_and_zero_knowledge() {
        // The headline V10 property when combined with payload.rs:
        // the row's plaintext travels through the outbox as
        // ciphertext, but the server-routing metadata (entity,
        // hlc, op_id) stays plaintext. After decrypt, the record
        // is byte-identical.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);
        let rec = fixture_record();
        let key = dek();

        let plain_entry = encode_command_creation(&rec).unwrap();
        let plain_payload = plain_entry.payload.clone();

        let sealed = encrypt_entry(plain_entry, &key).unwrap();
        assert!(sealed.encrypted);
        assert_ne!(sealed.payload, plain_payload);
        // Server-routing metadata survives the wrap unchanged.
        assert_eq!(sealed.entity, ACTUATOR_COMMANDS_ENTITY);
        assert_eq!(sealed.op_id, rec.id.to_string());

        outbox.enqueue(sealed).await.unwrap();
        let mut polled = outbox.poll(10).await.unwrap();
        let stored = polled.pop().unwrap();
        assert!(stored.encrypted);
        // Plaintext does NOT leak through the storage path.
        assert!(!stored
            .payload
            .windows(rec.command_kind.len())
            .any(|w| w == rec.command_kind.as_bytes()));

        let recovered_bytes = decrypt_entry(&stored, &key).unwrap();
        let decoded: ActuatorCommandRecord = serde_json::from_slice(&recovered_bytes).unwrap();
        assert_eq!(decoded, rec);
    }

    #[tokio::test]
    async fn interleaved_creation_and_transitions_each_get_a_row_in_outbox() {
        // The V10 sync story: a single actuator command's full
        // lifecycle pushes one Insert + N Updates through the
        // outbox. Each entry is independently durable, so a crash
        // between transitions doesn't corrupt the server view —
        // the worst case is "we replay an Update the server
        // already saw," which the HLC LWW reconciler collapses.
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = SqliteOutbox::new(pool);

        let mut rec = fixture_record();
        let create = encode_command_creation(&rec).unwrap();
        outbox.enqueue(create).await.unwrap();

        let parent = rec.hlc.clone();
        rec.status = ActuatorCommandStatus::InFlight;
        rec.hlc = Hlc::new(43, 0, "node-a");
        rec.started_at = Some(Utc::now());
        outbox
            .enqueue(encode_status_transition(&rec, parent.clone()).unwrap())
            .await
            .unwrap();

        let parent = rec.hlc.clone();
        rec.status = ActuatorCommandStatus::Completed;
        rec.hlc = Hlc::new(44, 0, "node-a");
        rec.completed_at = Some(Utc::now());
        rec.result = Some(serde_json::json!({"ok": true}));
        outbox
            .enqueue(encode_status_transition(&rec, parent).unwrap())
            .await
            .unwrap();

        let polled = outbox.poll(10).await.unwrap();
        assert_eq!(polled.len(), 3);
        // Same entity_id across all three so the server collapses
        // them onto the same row.
        let id_str = rec.id.to_string();
        for e in &polled {
            assert_eq!(e.entity_id, id_str);
        }
        // Ops descend the expected sequence.
        let ops: Vec<_> = polled.iter().map(|e| e.op.clone()).collect();
        // SqliteOutbox::poll doesn't guarantee insertion order in
        // its API, but the in-memory backing returns FIFO. Pin
        // the order we observe so a future change makes a noise.
        assert_eq!(ops, vec![Op::Insert, Op::Update, Op::Update]);
    }
}
