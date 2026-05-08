//! Reconciliation algorithm.
//!
//! Critical correctness boundary — see plan and `docs/architecture/sync.md`.
//! The skeleton defines the surface only; the algorithm itself ships in PR #2
//! together with property-based tests for convergence.

use crate::outbox::OutboxEntry;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    /// Default: last-writer-wins by HLC.
    LastWriterWinsHlc,
    /// Inventory & finance — reject and surface to UI for human resolution.
    RejectAndSurface,
    /// CRDT documents — merge via Automerge.
    AutomergeMerge,
    /// State machines — server-side function with `current_state = from` check.
    ServerTransition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub entity: String,
    pub entity_id: String,
    pub local: OutboxEntry,
    pub remote: OutboxEntry,
    pub policy: ConflictPolicy,
}

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("network: {0}")]
    Network(String),
    #[error("conflict requires user action")]
    UserActionRequired(Conflict),
    #[error("internal: {0}")]
    Internal(String),
}

/// Reconciler abstracts the push/pull pipeline against the server.
/// Implementations are backed by Supabase RPC + Realtime in production.
#[async_trait::async_trait]
pub trait Reconciler: Send + Sync {
    async fn push(&self, entries: &[OutboxEntry]) -> Result<PushResult, ReconcilerError>;
    async fn pull_since(&self, hlc: Option<&str>) -> Result<Vec<OutboxEntry>, ReconcilerError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PushResult {
    pub accepted: Vec<String>,
    pub conflicts: Vec<Conflict>,
}

/// Resolve which `ConflictPolicy` applies to a given entity. Centralizing
/// this lookup ensures the "inventory writes via RPC only" invariant is
/// not silently bypassed by adding a new sync path.
pub fn policy_for(entity: &str) -> ConflictPolicy {
    match entity {
        "materials" => ConflictPolicy::RejectAndSurface,
        "financial_ledger" => ConflictPolicy::RejectAndSurface,
        "boms" | "sop_drafts" => ConflictPolicy::AutomergeMerge,
        "work_orders" => ConflictPolicy::ServerTransition,
        _ => ConflictPolicy::LastWriterWinsHlc,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_uses_reject_and_surface() {
        assert_eq!(policy_for("materials"), ConflictPolicy::RejectAndSurface);
    }

    #[test]
    fn boms_use_automerge() {
        assert_eq!(policy_for("boms"), ConflictPolicy::AutomergeMerge);
    }

    #[test]
    fn unknown_falls_back_to_lww() {
        assert_eq!(policy_for("unknown_entity"), ConflictPolicy::LastWriterWinsHlc);
    }
}
