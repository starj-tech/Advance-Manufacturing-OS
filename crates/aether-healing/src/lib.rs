//! Neural Auto-Healing — diagnose, decide, act, audit.
//!
//! Captures runtime symptoms (panics, sync failures, repeated bridge
//! disconnects, SQLite WAL corruption, telemetry overflow) and routes
//! each through a `Healer` chain. Each `Healer` returns a `Diagnosis`;
//! the engine selects the highest-confidence diagnosis and applies the
//! associated `HealingPolicy`.
//!
//! Every action is appended to `HealingLedger` so operators can audit
//! exactly what the system did on their behalf. We never silently mutate
//! production data — destructive remediations require an Edge Function
//! confirmation in the cloud (PR #5).
//!
//! ## Skeleton scope
//! Trait surface, types, and a `NoopHealer` test impl. Real healers
//! (sync rebuild, log compaction, version rollback) ship with PR #5.

pub mod diagnosis;
pub mod dispatcher;
pub mod ledger;
pub mod policy;
pub mod sync_stuck;

pub use diagnosis::{Diagnosis, Healer, HealerError, Symptom};
pub use dispatcher::Dispatcher;
pub use ledger::{HealingEvent, HealingLedger, HealingOutcome};
pub use policy::HealingPolicy;
pub use sync_stuck::SyncStuckHealer;
