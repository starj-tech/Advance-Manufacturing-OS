//! Applier — local side of the sync engine's pull pipeline.
//!
//! When `SyncEngine::pull_once` fetches `OutboxEntry`s from the
//! reconciler, each one must be written into the client's local state
//! (the per-tenant SQLite domain tables, or a CRDT document, or an
//! encrypted field, depending on the entity). That side-effecting step
//! is delegated to an `Applier` implementation.
//!
//! Why a trait rather than direct writes?
//!   1. Production wires several appliers, one per entity family
//!      (work_orders → state-machine applier, materials →
//!      inventory_adjust replay, boms → Automerge merger).
//!   2. Property tests need a fast in-memory applier (`MemoryApplier`)
//!      so convergence assertions don't have to interrogate SQLite.
//!   3. Future modules (`aether-modules`) can register their own
//!      appliers for module-owned entities without touching core sync
//!      code.
//!
//! The trait returns `Applied | Skipped` so the engine can surface
//! statistics (newer-than-local writes vs LWW-loser drops).

use crate::outbox::OutboxEntry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Outcome of applying one pulled entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// Entry was newer than local state — local state updated.
    Applied,
    /// Entry was older than what we already had — dropped (LWW loser).
    Skipped,
}

#[derive(Debug, Error)]
pub enum ApplierError {
    #[error("storage: {0}")]
    Storage(String),
    #[error("encoding: {0}")]
    Encoding(String),
}

/// Apply a single entry pulled from the reconciler to local state.
///
/// Implementations MUST be idempotent: applying the same entry twice
/// MUST yield the same final state. Concretely: compare the entry's
/// HLC against whatever the local state holds and refuse to regress.
#[async_trait::async_trait]
pub trait Applier: Send + Sync {
    async fn apply(&self, entry: &OutboxEntry) -> Result<ApplyOutcome, ApplierError>;
}

/// In-memory LWW applier — keeps a `HashMap<(entity, entity_id), entry>`
/// and applies the standard "higher HLC wins" rule. Used in tests and
/// as a reference implementation for the production SQLite applier.
///
/// Cheaply clonable: every clone shares the same `Arc<Mutex<...>>`
/// state, mirroring the `LocalReconciler` pattern.
#[derive(Clone, Default)]
pub struct MemoryApplier {
    state: Arc<Mutex<HashMap<(String, String), OutboxEntry>>>,
}

impl MemoryApplier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self, entity: &str, entity_id: &str) -> Option<OutboxEntry> {
        self.state
            .lock()
            .expect("MemoryApplier mutex poisoned")
            .get(&(entity.to_string(), entity_id.to_string()))
            .cloned()
    }

    pub fn row_count(&self) -> usize {
        self.state
            .lock()
            .expect("MemoryApplier mutex poisoned")
            .len()
    }
}

#[async_trait::async_trait]
impl Applier for MemoryApplier {
    async fn apply(&self, entry: &OutboxEntry) -> Result<ApplyOutcome, ApplierError> {
        let mut state = self.state.lock().expect("MemoryApplier mutex poisoned");
        let key = (entry.entity.clone(), entry.entity_id.clone());
        match state.get(&key) {
            Some(existing) if existing.hlc_ts >= entry.hlc_ts => Ok(ApplyOutcome::Skipped),
            _ => {
                state.insert(key, entry.clone());
                Ok(ApplyOutcome::Applied)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Op;
    use aether_core::Hlc;

    fn entry(op_id: &str, entity_id: &str, hlc: Hlc) -> OutboxEntry {
        OutboxEntry {
            op_id: op_id.into(),
            entity: "work_orders".into(),
            entity_id: entity_id.into(),
            op: Op::Update,
            payload: vec![],
            hlc_ts: hlc,
            parent_hlc: None,
            encrypted: false,
        }
    }

    #[tokio::test]
    async fn fresh_entry_is_applied() {
        let a = MemoryApplier::new();
        let outcome = a
            .apply(&entry("op-1", "wo-1", Hlc::new(1, 0, "node-a")))
            .await
            .unwrap();
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(a.row_count(), 1);
    }

    #[tokio::test]
    async fn older_entry_is_skipped() {
        let a = MemoryApplier::new();
        a.apply(&entry("op-late", "wo-1", Hlc::new(10, 0, "node-a")))
            .await
            .unwrap();
        let outcome = a
            .apply(&entry("op-early", "wo-1", Hlc::new(5, 0, "node-a")))
            .await
            .unwrap();
        assert_eq!(outcome, ApplyOutcome::Skipped);

        // Local state should still hold the later entry.
        let cur = a.current("work_orders", "wo-1").unwrap();
        assert_eq!(cur.op_id, "op-late");
    }

    #[tokio::test]
    async fn equal_hlc_is_skipped_idempotent() {
        // Idempotence: applying the same entry twice MUST leave state unchanged.
        let a = MemoryApplier::new();
        let h = Hlc::new(7, 0, "node-a");
        let first = a.apply(&entry("op-1", "wo-1", h.clone())).await.unwrap();
        let second = a.apply(&entry("op-1", "wo-1", h)).await.unwrap();
        assert_eq!(first, ApplyOutcome::Applied);
        assert_eq!(second, ApplyOutcome::Skipped);
        assert_eq!(a.row_count(), 1);
    }

    #[tokio::test]
    async fn newer_entry_overwrites_existing() {
        let a = MemoryApplier::new();
        a.apply(&entry("op-early", "wo-1", Hlc::new(5, 0, "node-a")))
            .await
            .unwrap();
        let outcome = a
            .apply(&entry("op-late", "wo-1", Hlc::new(10, 0, "node-a")))
            .await
            .unwrap();
        assert_eq!(outcome, ApplyOutcome::Applied);
        let cur = a.current("work_orders", "wo-1").unwrap();
        assert_eq!(cur.op_id, "op-late");
    }

    #[tokio::test]
    async fn different_keys_are_independent() {
        let a = MemoryApplier::new();
        a.apply(&entry("op-1", "wo-1", Hlc::new(1, 0, "node-a")))
            .await
            .unwrap();
        a.apply(&entry("op-2", "wo-2", Hlc::new(2, 0, "node-a")))
            .await
            .unwrap();
        a.apply(&entry("op-3", "wo-3", Hlc::new(3, 0, "node-a")))
            .await
            .unwrap();
        assert_eq!(a.row_count(), 3);
    }

    #[tokio::test]
    async fn shared_state_visible_across_clones() {
        let a1 = MemoryApplier::new();
        let a2 = a1.clone();
        a1.apply(&entry("op-1", "wo-1", Hlc::new(1, 0, "node-a")))
            .await
            .unwrap();
        assert_eq!(
            a2.row_count(),
            1,
            "clones must share state for multi-client tests"
        );
    }
}
