//! Sync engine — ties together the three primitives:
//!
//! ```text
//!   App write ──► Outbox::enqueue (local SQLite, durable)
//!                       │
//!                       ▼
//!              SyncEngine::push_once
//!                       │
//!         ┌─────────────┼─────────────┐
//!         ▼             ▼             ▼
//!   Outbox::poll   Reconciler.push   Outbox::mark_done / mark_failed
//! ```
//!
//! `push_once` is the smallest useful unit of progress: one batch of
//! up to `batch_size` entries is pulled from the outbox, handed to the
//! reconciler, and the result demultiplexed back into the outbox state
//! (accepted → mark_done; conflicted → mark_failed with reason).
//!
//! Drives in production are layered on top: a background task that
//! loops `push_once` with backoff, a kill-switch via tenant flag, etc.
//! Keeping the unit small makes property testing tractable.

use crate::outbox::{Outbox, OutboxError};
use crate::reconcile::{Reconciler, ReconcilerError};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("outbox: {0}")]
    Outbox(#[from] OutboxError),
    #[error("reconciler: {0}")]
    Reconciler(String),
}

impl From<ReconcilerError> for EngineError {
    fn from(e: ReconcilerError) -> Self {
        EngineError::Reconciler(e.to_string())
    }
}

/// Counts surfaced from a single `push_once` call. Used by tests and
/// observability — production wires these to OpenTelemetry counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PushStats {
    pub polled: usize,
    pub accepted: usize,
    pub conflicts: usize,
}

/// Couples an `Outbox` (local durable queue) with a `Reconciler`
/// (server transport). Both are behind dynamic-dispatch trait objects
/// so the same engine drives in-memory tests and production Supabase
/// reconciliation without changes.
pub struct SyncEngine {
    outbox: Arc<dyn Outbox>,
    reconciler: Arc<dyn Reconciler>,
}

impl SyncEngine {
    pub fn new(outbox: Arc<dyn Outbox>, reconciler: Arc<dyn Reconciler>) -> Self {
        Self { outbox, reconciler }
    }

    /// One push round: poll up to `batch_size` entries, push them via
    /// the reconciler, then update the outbox state based on the
    /// result. Returns counters so the caller can decide whether to
    /// loop (more polled than 0 → more work may exist).
    pub async fn push_once(&self, batch_size: u32) -> Result<PushStats, EngineError> {
        let batch = self.outbox.poll(batch_size).await?;
        if batch.is_empty() {
            return Ok(PushStats::default());
        }
        let polled = batch.len();
        let result = self.reconciler.push(&batch).await?;

        let accepted = result.accepted.len();
        let conflicts = result.conflicts.len();

        if !result.accepted.is_empty() {
            self.outbox.mark_done(&result.accepted).await?;
        }

        for c in &result.conflicts {
            // Persist the failure reason against the LOCAL op_id so the
            // operator can inspect why a row is still queued. We don't
            // mark_done a conflicted entry — it stays in the outbox
            // until the user resolves it (e.g. via the Manager SupportPage).
            self.outbox
                .mark_failed(
                    &c.local.op_id,
                    &format!("conflict: policy={:?} entity={}", c.policy, c.entity),
                )
                .await?;
        }

        tracing::debug!(
            polled,
            accepted,
            conflicts,
            "sync engine push_once complete"
        );

        Ok(PushStats {
            polled,
            accepted,
            conflicts,
        })
    }

    /// Push in a loop until the outbox is empty or `max_rounds` rounds
    /// have run. Returns the accumulated stats. Tests use this to
    /// drain a queue deterministically; production layers backoff on
    /// top with `tokio::time::sleep` between rounds.
    pub async fn drain(&self, batch_size: u32, max_rounds: u32) -> Result<PushStats, EngineError> {
        let mut total = PushStats::default();
        for _ in 0..max_rounds {
            let round = self.push_once(batch_size).await?;
            if round.polled == 0 {
                break;
            }
            total.polled += round.polled;
            total.accepted += round.accepted;
            total.conflicts += round.conflicts;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlc::{HlcGenerator, WallClock};
    use crate::local_reconciler::LocalReconciler;
    use crate::outbox::{Op, OutboxEntry};
    use crate::sqlite_outbox::SqliteOutbox;
    use aether_core::Hlc;
    use aether_db::Pool;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct StubClock(AtomicU64);
    impl StubClock {
        fn new(t: u64) -> Self {
            Self(AtomicU64::new(t))
        }
        fn advance(&self, by: u64) {
            self.0.fetch_add(by, Ordering::SeqCst);
        }
    }
    impl WallClock for StubClock {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    /// Spin up a self-contained client: SQLite outbox + HLC generator
    /// for one node, sharing the `reconciler` (server) with other
    /// clients in the same test.
    async fn client(
        node: &'static str,
        start_clock: u64,
        reconciler: LocalReconciler,
    ) -> (Arc<SqliteOutbox>, HlcGenerator, SyncEngine) {
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = Arc::new(SqliteOutbox::new(pool));
        let hlc = HlcGenerator::with_clock(node, StubClock::new(start_clock));
        let engine = SyncEngine::new(outbox.clone(), Arc::new(reconciler));
        (outbox, hlc, engine)
    }

    fn entry_for(op_id: &str, entity: &str, entity_id: &str, op: Op, hlc: Hlc) -> OutboxEntry {
        OutboxEntry {
            op_id: op_id.into(),
            entity: entity.into(),
            entity_id: entity_id.into(),
            op,
            payload: vec![],
            hlc_ts: hlc,
            parent_hlc: None,
            encrypted: false,
        }
    }

    #[tokio::test]
    async fn push_once_on_empty_outbox_is_noop() {
        let server = LocalReconciler::new();
        let (_outbox, _hlc, engine) = client("node-a", 0, server).await;
        let stats = engine.push_once(100).await.unwrap();
        assert_eq!(stats, PushStats::default());
    }

    #[tokio::test]
    async fn push_once_drains_one_batch_and_marks_done() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine) = client("node-a", 1000, server.clone()).await;

        // Enqueue 3 work-order ops.
        for i in 0..3 {
            let h = hlc.next();
            outbox
                .enqueue(entry_for(
                    &format!("op-{}", i),
                    "work_orders",
                    &format!("wo-{}", i),
                    Op::Insert,
                    h,
                ))
                .await
                .unwrap();
        }
        assert_eq!(outbox.size().await.unwrap(), 3);

        let stats = engine.push_once(10).await.unwrap();
        assert_eq!(stats.polled, 3);
        assert_eq!(stats.accepted, 3);
        assert_eq!(stats.conflicts, 0);
        assert_eq!(
            outbox.size().await.unwrap(),
            0,
            "outbox must drain on accept"
        );
        assert_eq!(server.row_count(), 3, "server must have 3 work-order rows");
    }

    #[tokio::test]
    async fn conflict_keeps_entry_in_outbox_and_marks_failed() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine) = client("node-a", 1000, server.clone()).await;

        // First write to materials/mat-1 is an insert — accepted.
        let h1 = hlc.next();
        outbox
            .enqueue(entry_for("op-1", "materials", "mat-1", Op::Insert, h1))
            .await
            .unwrap();
        engine.push_once(10).await.unwrap();
        assert_eq!(outbox.size().await.unwrap(), 0);

        // Second write to the same row — RejectAndSurface.
        let h2 = hlc.next();
        outbox
            .enqueue(entry_for("op-2", "materials", "mat-1", Op::Update, h2))
            .await
            .unwrap();
        let stats = engine.push_once(10).await.unwrap();

        assert_eq!(stats.polled, 1);
        assert_eq!(stats.accepted, 0);
        assert_eq!(stats.conflicts, 1);
        assert_eq!(
            outbox.size().await.unwrap(),
            1,
            "conflicted entry stays in outbox awaiting resolution"
        );
    }

    #[tokio::test]
    async fn drain_loops_until_outbox_empty() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine) = client("node-a", 1000, server.clone()).await;

        // Enqueue 25 entries; batch size 10 forces 3 rounds.
        for i in 0..25 {
            let h = hlc.next();
            outbox
                .enqueue(entry_for(
                    &format!("op-{:02}", i),
                    "work_orders",
                    &format!("wo-{}", i),
                    Op::Insert,
                    h,
                ))
                .await
                .unwrap();
        }

        let stats = engine.drain(10, 10).await.unwrap();
        assert_eq!(stats.polled, 25);
        assert_eq!(stats.accepted, 25);
        assert_eq!(outbox.size().await.unwrap(), 0);
        assert_eq!(server.row_count(), 25);
    }

    /// Two clients write to the SAME row with different HLCs (different
    /// nodes, advancing clocks). After both push and drain, the server
    /// must hold exactly one row: the entry with the maximum HLC.
    /// Order of push_once interleaving must not matter.
    #[tokio::test]
    async fn two_clients_converge_on_higher_hlc_winner() {
        let server = LocalReconciler::new();
        let (ob_a, hlc_a, engine_a) = client("node-a", 1000, server.clone()).await;
        let (ob_b, hlc_b, engine_b) = client("node-b", 2000, server.clone()).await;

        // node-b's clock starts higher → its HLC values are strictly
        // greater than node-a's. Both clients independently propose an
        // update to work_orders/wo-1.
        let h_a = hlc_a.next();
        let h_b = hlc_b.next();
        assert!(h_b > h_a);

        ob_a.enqueue(entry_for("op-a", "work_orders", "wo-1", Op::Update, h_a))
            .await
            .unwrap();
        ob_b.enqueue(entry_for(
            "op-b",
            "work_orders",
            "wo-1",
            Op::Update,
            h_b.clone(),
        ))
        .await
        .unwrap();

        // Push in opposite order to what the HLC would suggest — A first.
        engine_a.push_once(10).await.unwrap();
        engine_b.push_once(10).await.unwrap();

        let cur = server.current("work_orders", "wo-1").unwrap();
        assert_eq!(
            cur.op_id, "op-b",
            "higher HLC must win regardless of push order"
        );
        assert_eq!(cur.hlc_ts, h_b);
    }

    // Property: For any random sequence of (client, entity_id) writes
    // from two clients with distinct clocks, after both drain their
    // outboxes the server's state for each entity_id must equal the
    // entry with the highest HLC across all writes to that key.
    // This is the core LWW convergence guarantee.
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(48))]
        #[test]
        fn two_clients_converge_under_arbitrary_interleaving(
            // Each tuple: (which_client 0|1, entity_id_index 0..5, clock_bump 1..50)
            ops in proptest::collection::vec(
                (proptest::bool::ANY, 0u8..5, 1u64..50),
                1..40,
            ),
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
                let server = LocalReconciler::new();

                let pool_a = Pool::open_in_memory().await.unwrap();
                let pool_b = Pool::open_in_memory().await.unwrap();
                let outbox_a: Arc<dyn Outbox> = Arc::new(SqliteOutbox::new(pool_a));
                let outbox_b: Arc<dyn Outbox> = Arc::new(SqliteOutbox::new(pool_b));
                let clock_a = std::sync::Arc::new(StubClock::new(1000));
                let clock_b = std::sync::Arc::new(StubClock::new(2000));

                struct ArcClock(std::sync::Arc<StubClock>);
                impl WallClock for ArcClock {
                    fn now_ms(&self) -> u64 { self.0.now_ms() }
                }

                let hlc_a = HlcGenerator::with_clock("node-a", ArcClock(std::sync::Arc::clone(&clock_a)));
                let hlc_b = HlcGenerator::with_clock("node-b", ArcClock(std::sync::Arc::clone(&clock_b)));

                let engine_a = SyncEngine::new(outbox_a.clone(), Arc::new(server.clone()));
                let engine_b = SyncEngine::new(outbox_b.clone(), Arc::new(server.clone()));

                // Ground-truth: for each entity_id, the entry with max HLC.
                // We track this in parallel as the test generates ops.
                let mut expected: std::collections::HashMap<String, Hlc> =
                    std::collections::HashMap::new();

                for (i, (use_b, ent, bump)) in ops.iter().enumerate() {
                    let entity_id = format!("wo-{}", ent);
                    let (ob, hlc, clock) = if *use_b {
                        (&outbox_b, &hlc_b, &clock_b)
                    } else {
                        (&outbox_a, &hlc_a, &clock_a)
                    };
                    clock.advance(*bump);
                    let h = hlc.next();
                    let op_id = format!("op-{:04}-{}", i, if *use_b { "b" } else { "a" });
                    ob.enqueue(entry_for(&op_id, "work_orders", &entity_id, Op::Update, h.clone()))
                        .await
                        .unwrap();

                    // Update ground truth.
                    expected
                        .entry(entity_id)
                        .and_modify(|cur| { if h > *cur { *cur = h.clone(); } })
                        .or_insert(h);
                }

                // Interleave drain rounds so neither client gets to
                // monopolize the server.
                for _ in 0..ops.len() {
                    engine_a.push_once(3).await.unwrap();
                    engine_b.push_once(3).await.unwrap();
                }
                engine_a.drain(100, 50).await.unwrap();
                engine_b.drain(100, 50).await.unwrap();

                // Final assertion: every key on the server matches the
                // max-HLC ground truth.
                for (entity_id, expected_hlc) in &expected {
                    let cur = server.current("work_orders", entity_id).unwrap_or_else(|| {
                        panic!("expected server to hold key {}, found none", entity_id)
                    });
                    proptest::prop_assert_eq!(
                        &cur.hlc_ts,
                        expected_hlc,
                        "convergence violated for {}: server={:?} expected={:?}",
                        entity_id,
                        cur.hlc_ts,
                        expected_hlc
                    );
                }
                Ok(())
            })?;
        }
    }
}
