//! Local-first sync engine.
//!
//! ## Pattern
//! Hybrid: outbox + Hybrid Logical Clock (HLC) for transactional data,
//! Automerge CRDT for collaborative documents (BOM, SOP). See
//! `docs/architecture/sync.md`.
//!
//! ## Conflict resolution
//!
//! - **Counter** (e.g. `materials.qty_on_hand`): client never updates the
//!   absolute value — it submits a delta via the server RPC
//!   `inventory_adjust(delta)` which enforces `qty + delta >= 0` atomically.
//! - **State machine** (e.g. `work_orders.status`): server function
//!   `wo_transition(id, from, to)` checks `current_state = from`.
//! - **Document** (BOM/SOP): Automerge CRDT.
//! - **Default**: last-writer-wins by HLC.
//!
//! ## Skeleton scope
//! This PR ships the public API surface and types. The reconciliation
//! algorithm, conflict handlers, and Automerge integration land in PR #2.

pub mod actuator_command;
pub mod applier;
pub mod automerge_merge;
pub mod engine;
pub mod gateway_sample;
pub mod hlc;
pub mod http_reconciler;
pub mod local_reconciler;
pub mod metrics;
pub mod operator_event;
pub mod outbox;
pub mod payload;
pub mod reconcile;
pub mod resolve;
pub mod scan_event;
pub mod sqlite_outbox;

pub use actuator_command::{
    encode_command_creation, encode_status_transition, ActuatorCommandRecord,
    ActuatorCommandStatus, EncodingError as ActuatorCommandEncodingError, ACTUATOR_COMMANDS_ENTITY,
};
pub use applier::{Applier, ApplierError, ApplyOutcome, MemoryApplier};
pub use automerge_merge::{merge_documents, MergeError};
pub use engine::{EngineError, PullStats, PushStats, SyncEngine};
pub use gateway_sample::{
    encode_sample, GatewaySampleRecord, SampleEncodingError, GATEWAY_SAMPLES_ENTITY,
    MAX_SAMPLE_PAYLOAD_BYTES,
};
pub use hlc::{HlcError, HlcGenerator, SystemClock, WallClock};
pub use http_reconciler::HttpReconciler;
pub use local_reconciler::LocalReconciler;
pub use metrics::{MetricsSnapshot, SyncMetrics};
pub use operator_event::{
    encode_event, EventEncodingError, OperatorEventRecord, OPERATOR_EVENTS_ENTITY,
};
pub use outbox::{Op, Outbox, OutboxEntry, OutboxError};
pub use payload::{decrypt_entry, encrypt_entry, PayloadError};
pub use reconcile::{
    policy_for, Conflict, ConflictPolicy, PullResult, PushResult, Reconciler, ReconcilerError,
};
pub use resolve::{resolve, ConflictOutcome};
pub use scan_event::{
    encode_scan, ScanEncodingError, ScanEventRecord, MAX_SCAN_PAYLOAD_BYTES, SCAN_EVENTS_ENTITY,
};
pub use sqlite_outbox::SqliteOutbox;
