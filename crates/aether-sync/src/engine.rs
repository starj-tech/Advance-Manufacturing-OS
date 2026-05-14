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

use crate::applier::{Applier, ApplierError, ApplyOutcome};
use crate::outbox::{Outbox, OutboxError};
use crate::reconcile::{Reconciler, ReconcilerError};
use aether_core::Hlc;
use aether_db::Pool;
use std::sync::Arc;
use thiserror::Error;

/// Sentinel `sync_metadata.entity` row that holds the global pull
/// watermark. Using a sentinel rather than per-entity rows keeps the
/// pull cursor a single source of truth and avoids the partial-pull
/// failure mode where one entity's cursor advances but another's
/// regresses on retry.
const PULL_CURSOR_KEY: &str = "__pull__";

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("outbox: {0}")]
    Outbox(#[from] OutboxError),
    #[error("reconciler: {0}")]
    Reconciler(String),
    #[error("applier: {0}")]
    Applier(String),
    #[error("storage: {0}")]
    Storage(String),
}

impl From<ApplierError> for EngineError {
    fn from(e: ApplierError) -> Self {
        EngineError::Applier(e.to_string())
    }
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

/// Counts surfaced from a single `pull_once` call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PullStats {
    pub fetched: usize,
    pub applied: usize,
    pub skipped: usize,
}

/// Couples an `Outbox` (local durable queue) with a `Reconciler`
/// (server transport) and a `Pool` for cursor persistence. Both the
/// outbox and reconciler are behind dynamic-dispatch trait objects so
/// the same engine drives in-memory tests and production Supabase
/// reconciliation without changes.
///
/// The `pool` is required because the pull cursor (high-water mark
/// of `last_pulled_hlc`) is persisted to the `sync_metadata` table —
/// if the app crashes mid-pull, we MUST NOT re-pull from epoch on the
/// next boot, both for efficiency and for idempotence with the
/// reconciler's `pull_since` semantics.
pub struct SyncEngine {
    outbox: Arc<dyn Outbox>,
    reconciler: Arc<dyn Reconciler>,
    pool: Pool,
}

impl SyncEngine {
    pub fn new(outbox: Arc<dyn Outbox>, reconciler: Arc<dyn Reconciler>, pool: Pool) -> Self {
        Self {
            outbox,
            reconciler,
            pool,
        }
    }

    /// Read the persisted pull watermark, or `None` if the client has
    /// never pulled before. Wire format: HLC `wall.logical.node`.
    pub async fn load_pull_cursor(&self) -> Result<Option<Hlc>, EngineError> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT last_pulled_hlc FROM sync_metadata WHERE entity = ?1")
                .bind(PULL_CURSOR_KEY)
                .fetch_optional(self.pool.handle())
                .await
                .map_err(|e| EngineError::Storage(e.to_string()))?;

        let raw = match row.and_then(|(s,)| s) {
            Some(s) => s,
            None => return Ok(None),
        };
        Hlc::parse(&raw)
            .map(Some)
            .ok_or_else(|| EngineError::Storage(format!("malformed cursor `{}`", raw)))
    }

    /// UPSERT the pull watermark for the next pull_once call.
    async fn save_pull_cursor(&self, cursor: &Hlc) -> Result<(), EngineError> {
        sqlx::query(
            "INSERT INTO sync_metadata (entity, last_pulled_hlc, last_pushed_hlc) \
             VALUES (?1, ?2, NULL) \
             ON CONFLICT(entity) DO UPDATE SET last_pulled_hlc = excluded.last_pulled_hlc",
        )
        .bind(PULL_CURSOR_KEY)
        .bind(cursor.to_string())
        .execute(self.pool.handle())
        .await
        .map_err(|e| EngineError::Storage(e.to_string()))?;
        Ok(())
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

    /// One pull round:
    ///   1. Read the persisted pull cursor (`sync_metadata.last_pulled_hlc`).
    ///   2. Reconciler::pull_since(cursor) → ordered Vec<OutboxEntry>.
    ///   3. For each entry, applier.apply(entry) → Applied | Skipped.
    ///   4. Advance the cursor to the max HLC observed (even for
    ///      Skipped entries — they still count as "seen" so we don't
    ///      re-fetch them next time).
    ///   5. Persist the new cursor.
    ///
    /// Idempotence is guaranteed by two layers:
    ///   - Reconciler returns entries strictly above the cursor.
    ///   - Applier compares HLC against local state and refuses to
    ///     regress, so re-applying a duplicate entry is a no-op
    ///     ApplyOutcome::Skipped.
    ///
    /// If the apply step errors after some entries succeed, the cursor
    /// is NOT advanced — the next pull will replay the partial batch.
    /// Combined with idempotence this gives at-least-once semantics
    /// with deterministic recovery.
    pub async fn pull_once<A: Applier + ?Sized>(
        &self,
        applier: &A,
    ) -> Result<PullStats, EngineError> {
        let cursor = self.load_pull_cursor().await?;
        let cursor_str = cursor.as_ref().map(|h| h.to_string());
        let entries = self
            .reconciler
            .pull_since(cursor_str.as_deref())
            .await
            .map_err(EngineError::from)?;

        if entries.is_empty() {
            return Ok(PullStats::default());
        }

        let mut applied = 0usize;
        let mut skipped = 0usize;
        let mut max_seen: Option<Hlc> = cursor;

        for entry in &entries {
            match applier.apply(entry).await? {
                ApplyOutcome::Applied => applied += 1,
                ApplyOutcome::Skipped => skipped += 1,
            }
            max_seen = Some(match max_seen {
                Some(m) if m > entry.hlc_ts => m,
                _ => entry.hlc_ts.clone(),
            });
        }

        if let Some(new_cursor) = max_seen {
            self.save_pull_cursor(&new_cursor).await?;
        }

        tracing::debug!(
            fetched = entries.len(),
            applied,
            skipped,
            "sync engine pull_once complete"
        );

        Ok(PullStats {
            fetched: entries.len(),
            applied,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applier::MemoryApplier;
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
    /// clients in the same test. Returns the underlying pool so the
    /// caller can also build an `Applier` against it if needed.
    async fn client(
        node: &'static str,
        start_clock: u64,
        reconciler: LocalReconciler,
    ) -> (Arc<SqliteOutbox>, HlcGenerator, SyncEngine, Pool) {
        let pool = Pool::open_in_memory().await.unwrap();
        let outbox = Arc::new(SqliteOutbox::new(pool.clone()));
        let hlc = HlcGenerator::with_clock(node, StubClock::new(start_clock));
        let engine = SyncEngine::new(outbox.clone(), Arc::new(reconciler), pool.clone());
        (outbox, hlc, engine, pool)
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
        let (_outbox, _hlc, engine, _pool) = client("node-a", 0, server).await;
        let stats = engine.push_once(100).await.unwrap();
        assert_eq!(stats, PushStats::default());
    }

    #[tokio::test]
    async fn push_once_drains_one_batch_and_marks_done() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;

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
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;

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
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;

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
        let (ob_a, hlc_a, engine_a, _pool_a) = client("node-a", 1000, server.clone()).await;
        let (ob_b, hlc_b, engine_b, _pool_b) = client("node-b", 2000, server.clone()).await;

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

    // -------- Pull side --------

    #[tokio::test]
    async fn pull_once_on_empty_server_is_noop() {
        let server = LocalReconciler::new();
        let (_outbox, _hlc, engine, _pool) = client("node-a", 0, server).await;
        let applier = MemoryApplier::new();
        let stats = engine.pull_once(&applier).await.unwrap();
        assert_eq!(stats, PullStats::default());
        assert_eq!(applier.row_count(), 0);
        assert_eq!(
            engine.load_pull_cursor().await.unwrap(),
            None,
            "empty server pull must NOT persist a cursor"
        );
    }

    #[tokio::test]
    async fn pull_once_applies_all_entries_above_cursor() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;
        let applier = MemoryApplier::new();

        // Push 3 entries to the server first.
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
        engine.drain(10, 5).await.unwrap();

        // Now pull: applier was empty so all 3 are Applied.
        let stats = engine.pull_once(&applier).await.unwrap();
        assert_eq!(stats.fetched, 3);
        assert_eq!(stats.applied, 3);
        assert_eq!(stats.skipped, 0);
        assert_eq!(applier.row_count(), 3);

        // Cursor persisted; second pull is a no-op.
        let stats2 = engine.pull_once(&applier).await.unwrap();
        assert_eq!(stats2, PullStats::default());
    }

    #[tokio::test]
    async fn pull_once_advances_cursor_past_max_hlc() {
        let server = LocalReconciler::new();
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;
        let applier = MemoryApplier::new();

        let mut highest = None;
        for i in 0..5 {
            let h = hlc.next();
            outbox
                .enqueue(entry_for(
                    &format!("op-{}", i),
                    "work_orders",
                    &format!("wo-{}", i),
                    Op::Insert,
                    h.clone(),
                ))
                .await
                .unwrap();
            highest = Some(h);
        }
        engine.drain(100, 5).await.unwrap();
        engine.pull_once(&applier).await.unwrap();

        let cursor = engine.load_pull_cursor().await.unwrap();
        assert_eq!(
            cursor, highest,
            "cursor must equal the max HLC observed in the pulled batch"
        );
    }

    #[tokio::test]
    async fn pull_once_is_idempotent_against_local_state() {
        // Calling pull_once twice on the same server data must leave
        // the applier in the same state — duplicate entries are
        // ApplyOutcome::Skipped by MemoryApplier's HLC guard.
        let server = LocalReconciler::new();
        let (outbox, hlc, engine, _pool) = client("node-a", 1000, server.clone()).await;
        let applier = MemoryApplier::new();

        outbox
            .enqueue(entry_for(
                "op-1",
                "work_orders",
                "wo-1",
                Op::Insert,
                hlc.next(),
            ))
            .await
            .unwrap();
        engine.drain(10, 5).await.unwrap();

        let first = engine.pull_once(&applier).await.unwrap();
        assert_eq!(first.applied, 1);

        // Manually rewind cursor to force a second fetch + apply.
        // Real production never rewinds, but this verifies the applier
        // half-idempotence: re-applying the SAME entry is a Skipped no-op.
        sqlx::query("DELETE FROM sync_metadata WHERE entity = '__pull__'")
            .execute(_pool.handle())
            .await
            .unwrap();
        let second = engine.pull_once(&applier).await.unwrap();
        assert_eq!(second.fetched, 1);
        assert_eq!(second.applied, 0);
        assert_eq!(second.skipped, 1);
        assert_eq!(applier.row_count(), 1, "no duplicate row created");
    }

    #[tokio::test]
    async fn two_clients_see_each_others_pushes_after_pull() {
        let server = LocalReconciler::new();
        let (ob_a, hlc_a, engine_a, _pool_a) = client("node-a", 1000, server.clone()).await;
        let (ob_b, hlc_b, engine_b, _pool_b) = client("node-b", 2000, server.clone()).await;
        let applier_a = MemoryApplier::new();
        let applier_b = MemoryApplier::new();

        // node-a writes wo-1, node-b writes wo-2.
        ob_a.enqueue(entry_for(
            "op-a",
            "work_orders",
            "wo-1",
            Op::Insert,
            hlc_a.next(),
        ))
        .await
        .unwrap();
        ob_b.enqueue(entry_for(
            "op-b",
            "work_orders",
            "wo-2",
            Op::Insert,
            hlc_b.next(),
        ))
        .await
        .unwrap();

        engine_a.drain(10, 5).await.unwrap();
        engine_b.drain(10, 5).await.unwrap();

        // Each client pulls — both see both writes.
        engine_a.pull_once(&applier_a).await.unwrap();
        engine_b.pull_once(&applier_b).await.unwrap();

        assert_eq!(applier_a.row_count(), 2);
        assert_eq!(applier_b.row_count(), 2);
        assert!(applier_a.current("work_orders", "wo-1").is_some());
        assert!(applier_a.current("work_orders", "wo-2").is_some());
        assert!(applier_b.current("work_orders", "wo-1").is_some());
        assert!(applier_b.current("work_orders", "wo-2").is_some());
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
                let outbox_a: Arc<dyn Outbox> = Arc::new(SqliteOutbox::new(pool_a.clone()));
                let outbox_b: Arc<dyn Outbox> = Arc::new(SqliteOutbox::new(pool_b.clone()));
                let clock_a = std::sync::Arc::new(StubClock::new(1000));
                let clock_b = std::sync::Arc::new(StubClock::new(2000));

                struct ArcClock(std::sync::Arc<StubClock>);
                impl WallClock for ArcClock {
                    fn now_ms(&self) -> u64 { self.0.now_ms() }
                }

                let hlc_a = HlcGenerator::with_clock("node-a", ArcClock(std::sync::Arc::clone(&clock_a)));
                let hlc_b = HlcGenerator::with_clock("node-b", ArcClock(std::sync::Arc::clone(&clock_b)));

                let engine_a = SyncEngine::new(outbox_a.clone(), Arc::new(server.clone()), pool_a);
                let engine_b = SyncEngine::new(outbox_b.clone(), Arc::new(server.clone()), pool_b);

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
