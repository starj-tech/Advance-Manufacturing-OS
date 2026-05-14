//! In-memory `Reconciler` implementation used as a test double for the
//! Supabase-backed production reconciler. It implements the LWW-by-HLC
//! semantics described in `docs/architecture/sync.md`, plus the
//! reject-and-surface path for inventory / financial entities.
//!
//! The implementation is intentionally short — its primary role is to
//! provide a deterministic server stand-in for property-based
//! convergence tests of the local client (Outbox + HlcGenerator).
//!
//! Real production behaviour lives in the future Supabase reconciler:
//!   * RPC `sync_apply_batch` consumes outbox entries
//!   * RPC `sync_changes_since` is the pull cursor
//!   * RLS enforces tenant isolation
//!   * `wo_transition` / `inventory_adjust` validate domain invariants
//!
//! `LocalReconciler` mirrors the wire contract so client code written
//! against the trait works unchanged when swapped to Supabase.

use crate::outbox::OutboxEntry;
use crate::reconcile::{
    policy_for, Conflict, ConflictPolicy, PullResult, PushResult, Reconciler, ReconcilerError,
};
#[cfg(test)]
use aether_core::Hlc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Snapshot of authoritative server state for an (entity, entity_id) row.
/// One entry per key, always the entry with the highest HLC observed so far.
#[derive(Clone, Debug)]
pub struct LocalServerRow {
    pub entry: OutboxEntry,
}

/// Log entry with the server-assigned sequence number. The cursor for
/// `pull_since` is this `seq`, NOT the entry's HLC: HLCs are local to
/// each node so `max(hlc)` across nodes is not monotonic, which would
/// cause a slow-clock node's later writes to be skipped by a pull
/// cursor advanced by a fast-clock node's earlier writes. Using a
/// server-assigned monotonic counter matches what Supabase production
/// would use (the `sync_changes.id BIGSERIAL`).
#[derive(Clone, Debug)]
struct LoggedEntry {
    seq: u64,
    entry: OutboxEntry,
}

#[derive(Default)]
struct ServerState {
    /// Authoritative per-row state. Key: `(entity, entity_id)`.
    current: HashMap<(String, String), OutboxEntry>,
    /// Append-only log of every accepted entry, in acceptance order
    /// (== server sequence number order).
    log: Vec<LoggedEntry>,
    /// Next sequence number to assign on push. Monotonic across all clients.
    next_seq: u64,
}

/// Test/integration server. Cheaply clonable: every clone shares the
/// same backing `Arc<Mutex<ServerState>>` so multiple `SyncEngine`s in
/// a test all see the same canonical state.
#[derive(Clone, Default)]
pub struct LocalReconciler {
    state: Arc<Mutex<ServerState>>,
}

impl LocalReconciler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the current authoritative row for a given (entity, entity_id).
    /// Used in tests to assert convergence without poking inside the mutex.
    pub fn current(&self, entity: &str, entity_id: &str) -> Option<OutboxEntry> {
        let g = self.state.lock().expect("LocalReconciler mutex poisoned");
        g.current
            .get(&(entity.to_string(), entity_id.to_string()))
            .cloned()
    }

    /// Number of distinct (entity, entity_id) keys currently tracked.
    pub fn row_count(&self) -> usize {
        self.state
            .lock()
            .expect("LocalReconciler mutex poisoned")
            .current
            .len()
    }

    /// Total entries seen across all clients (loser drops are not counted).
    pub fn log_len(&self) -> usize {
        self.state
            .lock()
            .expect("LocalReconciler mutex poisoned")
            .log
            .len()
    }

    /// Test helper: current value of the server's monotonic sequence counter.
    pub fn next_seq(&self) -> u64 {
        self.state
            .lock()
            .expect("LocalReconciler mutex poisoned")
            .next_seq
    }
}

#[async_trait::async_trait]
impl Reconciler for LocalReconciler {
    async fn push(&self, entries: &[OutboxEntry]) -> Result<PushResult, ReconcilerError> {
        let mut accepted: Vec<String> = Vec::with_capacity(entries.len());
        let mut conflicts: Vec<Conflict> = Vec::new();

        let mut state = self.state.lock().expect("LocalReconciler mutex poisoned");

        // Helper: append to the log with the next server seq. Hoisted
        // so every accept path increments the seq the same way.
        fn append(state: &mut ServerState, entry: &OutboxEntry) {
            let seq = state.next_seq;
            state.next_seq += 1;
            state.log.push(LoggedEntry {
                seq,
                entry: entry.clone(),
            });
        }

        for entry in entries {
            let key = (entry.entity.clone(), entry.entity_id.clone());
            let policy = policy_for(&entry.entity);

            match policy {
                ConflictPolicy::LastWriterWinsHlc => {
                    // LWW: compare HLC; the higher one wins. Loser is
                    // still "accepted" by the server in the sense that
                    // the client can drop it from its outbox — the
                    // server simply decided not to apply it. Losers
                    // are NOT appended to the log: pulls only return
                    // winners.
                    let should_apply = state
                        .current
                        .get(&key)
                        .map(|cur| entry.hlc_ts > cur.hlc_ts)
                        .unwrap_or(true);
                    if should_apply {
                        state.current.insert(key, entry.clone());
                        append(&mut state, entry);
                    }
                    accepted.push(entry.op_id.clone());
                }
                ConflictPolicy::RejectAndSurface => {
                    // Inventory / financial: ANY update conflicts because
                    // the absolute value must flow through a server RPC
                    // (inventory_adjust). Inserts are still OK.
                    if let Some(cur) = state.current.get(&key).cloned() {
                        conflicts.push(Conflict {
                            entity: entry.entity.clone(),
                            entity_id: entry.entity_id.clone(),
                            local: entry.clone(),
                            remote: cur,
                            policy,
                        });
                    } else {
                        state.current.insert(key, entry.clone());
                        append(&mut state, entry);
                        accepted.push(entry.op_id.clone());
                    }
                }
                ConflictPolicy::AutomergeMerge | ConflictPolicy::ServerTransition => {
                    // Both require domain logic that lives outside this
                    // in-memory test reconciler — wired in PR #2
                    // production slice. Fall back to LWW so the surface
                    // is exercised.
                    let should_apply = state
                        .current
                        .get(&key)
                        .map(|cur| entry.hlc_ts > cur.hlc_ts)
                        .unwrap_or(true);
                    if should_apply {
                        state.current.insert(key, entry.clone());
                        append(&mut state, entry);
                    }
                    accepted.push(entry.op_id.clone());
                }
            }
        }

        Ok(PushResult {
            accepted,
            conflicts,
        })
    }

    async fn pull_since(&self, cursor: Option<&str>) -> Result<PullResult, ReconcilerError> {
        // Cursor is the server-assigned seq number (u64 as string).
        // We don't use the entry's HLC for the cursor because HLC is
        // local-monotonic per node, not global-monotonic. A slow-clock
        // node's writes have lower wall_ms than a fast-clock node's
        // earlier writes, so cursoring on max(HLC) would skip the
        // slow-clock node's writes after the fast-clock node's pull
        // advanced the cursor. Server-assigned seq has no such hazard.
        // Production (Supabase) uses `sync_changes.id BIGSERIAL` the
        // same way.
        let parsed: Option<u64> = match cursor {
            Some(s) => Some(s.parse::<u64>().map_err(|_| {
                ReconcilerError::Internal(format!("malformed pull cursor `{}`", s))
            })?),
            None => None,
        };

        let state = self.state.lock().expect("LocalReconciler mutex poisoned");
        let filtered: Vec<&LoggedEntry> = state
            .log
            .iter()
            .filter(|e| match parsed {
                Some(c) => e.seq > c,
                None => true,
            })
            .collect();

        let next_cursor = filtered.last().map(|e| e.seq.to_string());
        let entries: Vec<OutboxEntry> = filtered.into_iter().map(|e| e.entry.clone()).collect();
        Ok(PullResult {
            entries,
            next_cursor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Op;

    fn entry(op_id: &str, entity: &str, entity_id: &str, hlc: Hlc) -> OutboxEntry {
        OutboxEntry {
            op_id: op_id.into(),
            entity: entity.into(),
            entity_id: entity_id.into(),
            op: Op::Update,
            payload: vec![],
            hlc_ts: hlc,
            parent_hlc: None,
            encrypted: false,
        }
    }

    #[tokio::test]
    async fn first_push_creates_row() {
        let r = LocalReconciler::new();
        let h = Hlc::new(1, 0, "node-a");
        let res = r
            .push(&[entry("op-1", "work_orders", "wo-1", h.clone())])
            .await
            .unwrap();
        assert_eq!(res.accepted, vec!["op-1"]);
        assert!(res.conflicts.is_empty());
        assert_eq!(r.row_count(), 1);
        assert_eq!(r.current("work_orders", "wo-1").unwrap().hlc_ts, h);
    }

    #[tokio::test]
    async fn lww_keeps_higher_hlc_regardless_of_push_order() {
        let r = LocalReconciler::new();
        let early = Hlc::new(10, 0, "node-a");
        let late = Hlc::new(20, 0, "node-b");

        // Push late first, then early.
        r.push(&[entry("op-late", "work_orders", "wo-1", late.clone())])
            .await
            .unwrap();
        r.push(&[entry("op-early", "work_orders", "wo-1", early)])
            .await
            .unwrap();

        let cur = r.current("work_orders", "wo-1").unwrap();
        assert_eq!(cur.hlc_ts, late);
        assert_eq!(cur.op_id, "op-late");
        // Only the winning entry made it into the log.
        assert_eq!(r.log_len(), 1);
    }

    #[tokio::test]
    async fn lww_overwrites_when_newer_pushed_second() {
        let r = LocalReconciler::new();
        let early = Hlc::new(10, 0, "node-a");
        let late = Hlc::new(20, 0, "node-b");

        r.push(&[entry("op-early", "work_orders", "wo-1", early)])
            .await
            .unwrap();
        r.push(&[entry("op-late", "work_orders", "wo-1", late.clone())])
            .await
            .unwrap();

        let cur = r.current("work_orders", "wo-1").unwrap();
        assert_eq!(cur.hlc_ts, late);
        assert_eq!(
            r.log_len(),
            2,
            "both entries reach the log when each was a winner at push time"
        );
    }

    #[tokio::test]
    async fn materials_update_after_insert_surfaces_conflict() {
        let r = LocalReconciler::new();
        let h1 = Hlc::new(1, 0, "node-a");
        let h2 = Hlc::new(2, 0, "node-a");

        // First push: a fresh row — insert path, accepted.
        let res = r
            .push(&[entry("op-1", "materials", "mat-1", h1)])
            .await
            .unwrap();
        assert_eq!(res.accepted, vec!["op-1"]);
        assert!(res.conflicts.is_empty());

        // Second push: row exists, materials is RejectAndSurface — conflict.
        let res = r
            .push(&[entry("op-2", "materials", "mat-1", h2)])
            .await
            .unwrap();
        assert!(res.accepted.is_empty());
        assert_eq!(res.conflicts.len(), 1);
        assert_eq!(res.conflicts[0].policy, ConflictPolicy::RejectAndSurface);
    }

    #[tokio::test]
    async fn pull_since_returns_log_above_cursor_in_seq_order() {
        // Server assigns seq 0, 1, 2 to the three winning entries (each
        // is a fresh row so LWW lets all three through).
        let r = LocalReconciler::new();
        for i in 1..=3 {
            r.push(&[entry(
                &format!("op-{}", i),
                "work_orders",
                &format!("wo-{}", i),
                Hlc::new(i * 10, 0, "node-a"),
            )])
            .await
            .unwrap();
        }

        let all = r.pull_since(None).await.unwrap();
        assert_eq!(all.entries.len(), 3);
        assert_eq!(all.next_cursor.as_deref(), Some("2"));

        let from_zero = r.pull_since(Some("0")).await.unwrap();
        assert_eq!(
            from_zero
                .entries
                .iter()
                .map(|e| e.op_id.as_str())
                .collect::<Vec<_>>(),
            vec!["op-2", "op-3"]
        );
        assert_eq!(from_zero.next_cursor.as_deref(), Some("2"));

        let from_last = r.pull_since(Some("2")).await.unwrap();
        assert!(from_last.entries.is_empty());
        assert!(from_last.next_cursor.is_none());
    }

    #[tokio::test]
    async fn pull_since_rejects_malformed_cursor() {
        let r = LocalReconciler::new();
        let err = r.pull_since(Some("garbage")).await;
        assert!(matches!(err, Err(ReconcilerError::Internal(_))));
    }

    #[tokio::test]
    async fn shared_state_visible_across_clones() {
        let r1 = LocalReconciler::new();
        let r2 = r1.clone();
        let h = Hlc::new(1, 0, "node-a");

        r1.push(&[entry("op-1", "work_orders", "wo-1", h)])
            .await
            .unwrap();

        // r2 shares the same underlying state — both clients in tests
        // see the same server.
        assert_eq!(r2.row_count(), 1);
    }
}
