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

pub mod hlc;
pub mod outbox;
pub mod reconcile;
pub mod sqlite_outbox;

pub use hlc::{HlcError, HlcGenerator, SystemClock, WallClock};
pub use outbox::{Op, Outbox, OutboxEntry, OutboxError};
pub use reconcile::{Conflict, ConflictPolicy, Reconciler, ReconcilerError};
pub use sqlite_outbox::SqliteOutbox;
