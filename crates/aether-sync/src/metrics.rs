//! Sync engine metrics — lock-free counters the engine
//! increments as it pushes / pulls / reconciles, exported
//! verbatim to whatever Prometheus / OpenTelemetry exporter
//! the binary wires up.
//!
//! ## Why pure atomics
//! The sync engine runs as a long-lived background task with
//! short, high-frequency hot paths (poll outbox, push batch,
//! apply pulled entries). Adding a Mutex around metrics
//! state would serialize every hot-path increment behind the
//! same lock. AtomicU64 is the canonical zero-overhead
//! counter primitive for this case.
//!
//! ## What this is NOT
//! A metrics export protocol. The exporter (Prometheus
//! pull, OTLP push, …) snapshots these counters and ships
//! them in whatever wire format the backend wants. This
//! module owns the increments; the exporter owns the
//! protocol.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Snapshot of all counter values at a point in time.
/// `MetricsSnapshot` derives Serialize so the desktop's
/// "system health" page can render it as JSON without
/// needing the live counter handle.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub outbox_enqueued: u64,
    pub outbox_pushed: u64,
    pub outbox_push_failures: u64,
    pub pull_batches: u64,
    pub pull_entries: u64,
    pub pull_failures: u64,
    pub conflicts_resolved_lww: u64,
    pub conflicts_resolved_remote_won: u64,
    pub conflicts_user_action_required: u64,
    pub conflicts_automerge: u64,
    pub conflicts_server_transition: u64,
}

/// Cloneable counter set. The engine constructs one
/// `SyncMetrics` at startup and clones it freely; clones
/// share the same backing atomics so every increment from
/// every task lands in the same totals.
#[derive(Clone, Default)]
pub struct SyncMetrics {
    inner: Arc<MetricsInner>,
}

#[derive(Default)]
struct MetricsInner {
    outbox_enqueued: AtomicU64,
    outbox_pushed: AtomicU64,
    outbox_push_failures: AtomicU64,
    pull_batches: AtomicU64,
    pull_entries: AtomicU64,
    pull_failures: AtomicU64,
    conflicts_resolved_lww: AtomicU64,
    conflicts_resolved_remote_won: AtomicU64,
    conflicts_user_action_required: AtomicU64,
    conflicts_automerge: AtomicU64,
    conflicts_server_transition: AtomicU64,
}

impl SyncMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Incremented once per `Outbox::enqueue` call. Pin
    /// against `outbox_pushed` to compute backlog depth.
    pub fn inc_outbox_enqueued(&self) {
        self.inner.outbox_enqueued.fetch_add(1, Ordering::Relaxed);
    }

    /// Incremented per successful push to the upstream
    /// reconciler. Add `n` for a batch push.
    pub fn add_outbox_pushed(&self, n: u64) {
        self.inner.outbox_pushed.fetch_add(n, Ordering::Relaxed);
    }

    /// Incremented per push failure (transport / server
    /// error). The healing layer subscribes; PR #5 wires the
    /// `sync.outbox.push_failures > threshold` symptom into
    /// `SyncStuckHealer`.
    pub fn inc_outbox_push_failures(&self) {
        self.inner
            .outbox_push_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Incremented per successful pull batch (one server
    /// round-trip with N entries).
    pub fn inc_pull_batches(&self) {
        self.inner.pull_batches.fetch_add(1, Ordering::Relaxed);
    }

    /// Add `n` for the entry count of a pull batch.
    pub fn add_pull_entries(&self, n: u64) {
        self.inner.pull_entries.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc_pull_failures(&self) {
        self.inner.pull_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Resolver outcome counters — one increment per
    /// conflict resolution. Sum of (lww + remote_won +
    /// user_action + automerge + server_transition) equals
    /// total conflicts observed.
    ///
    /// `lww` is the "local won" case under LWW;
    /// `remote_won` is the corresponding "remote won" case.
    /// Split for the dashboard view "are most conflicts
    /// local-stale or local-fresh?"
    pub fn inc_conflict_lww_local_won(&self) {
        self.inner
            .conflicts_resolved_lww
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_conflict_lww_remote_won(&self) {
        self.inner
            .conflicts_resolved_remote_won
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_conflict_user_action(&self) {
        self.inner
            .conflicts_user_action_required
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_conflict_automerge(&self) {
        self.inner
            .conflicts_automerge
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_conflict_server_transition(&self) {
        self.inner
            .conflicts_server_transition
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Atomic snapshot of every counter. The exporter calls
    /// this on its poll interval. The snapshot is a frozen
    /// copy; subsequent increments don't affect it.
    pub fn snapshot(&self) -> MetricsSnapshot {
        let i = &self.inner;
        MetricsSnapshot {
            outbox_enqueued: i.outbox_enqueued.load(Ordering::Relaxed),
            outbox_pushed: i.outbox_pushed.load(Ordering::Relaxed),
            outbox_push_failures: i.outbox_push_failures.load(Ordering::Relaxed),
            pull_batches: i.pull_batches.load(Ordering::Relaxed),
            pull_entries: i.pull_entries.load(Ordering::Relaxed),
            pull_failures: i.pull_failures.load(Ordering::Relaxed),
            conflicts_resolved_lww: i.conflicts_resolved_lww.load(Ordering::Relaxed),
            conflicts_resolved_remote_won: i.conflicts_resolved_remote_won.load(Ordering::Relaxed),
            conflicts_user_action_required: i
                .conflicts_user_action_required
                .load(Ordering::Relaxed),
            conflicts_automerge: i.conflicts_automerge.load(Ordering::Relaxed),
            conflicts_server_transition: i.conflicts_server_transition.load(Ordering::Relaxed),
        }
    }
}

impl MetricsSnapshot {
    /// Derived metric: outbox backlog depth = enqueued -
    /// pushed. Saturates at zero to handle the impossible-in-
    /// practice "more pushes than enqueues" case (would only
    /// happen with manual counter manipulation).
    pub fn outbox_backlog(&self) -> u64 {
        self.outbox_enqueued.saturating_sub(self.outbox_pushed)
    }

    /// Derived metric: total conflicts observed since start.
    pub fn total_conflicts(&self) -> u64 {
        self.conflicts_resolved_lww
            + self.conflicts_resolved_remote_won
            + self.conflicts_user_action_required
            + self.conflicts_automerge
            + self.conflicts_server_transition
    }

    /// Health predicate: returns true when push failures
    /// outpace push successes by 10x — the heuristic the
    /// healing layer's `SyncStuckHealer` uses to recognize a
    /// stuck-outbox symptom.
    pub fn is_pushing_in_distress(&self) -> bool {
        // Avoid the n/0 case: if we've never tried to push,
        // there's nothing in distress.
        if self.outbox_pushed == 0 && self.outbox_push_failures == 0 {
            return false;
        }
        // 10× threshold matches the PR #5 healing plan.
        self.outbox_push_failures > self.outbox_pushed.saturating_mul(10).max(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_metrics_zeroes_everywhere() {
        let m = SyncMetrics::new();
        let s = m.snapshot();
        assert_eq!(s, MetricsSnapshot::default());
        assert_eq!(s.outbox_backlog(), 0);
        assert_eq!(s.total_conflicts(), 0);
    }

    #[test]
    fn increments_show_up_in_snapshot() {
        let m = SyncMetrics::new();
        m.inc_outbox_enqueued();
        m.inc_outbox_enqueued();
        m.inc_outbox_enqueued();
        m.add_outbox_pushed(2);
        m.inc_outbox_push_failures();

        let s = m.snapshot();
        assert_eq!(s.outbox_enqueued, 3);
        assert_eq!(s.outbox_pushed, 2);
        assert_eq!(s.outbox_push_failures, 1);
        // Derived backlog = 3 - 2 = 1.
        assert_eq!(s.outbox_backlog(), 1);
    }

    #[test]
    fn metrics_clone_shares_state() {
        // The engine clones the metrics handle freely; every
        // clone increments the same counters.
        let m1 = SyncMetrics::new();
        let m2 = m1.clone();
        m1.inc_outbox_enqueued();
        m2.inc_outbox_enqueued();
        assert_eq!(m1.snapshot().outbox_enqueued, 2);
        assert_eq!(m2.snapshot().outbox_enqueued, 2);
    }

    #[test]
    fn snapshot_is_a_frozen_copy() {
        // Pin that snapshot() returns a CLONE — subsequent
        // increments don't affect it. The exporter relies on
        // this for consistent point-in-time views.
        let m = SyncMetrics::new();
        m.inc_outbox_enqueued();
        let s = m.snapshot();
        m.inc_outbox_enqueued();
        m.inc_outbox_enqueued();
        // Snapshot still says 1; live counter is now 3.
        assert_eq!(s.outbox_enqueued, 1);
        assert_eq!(m.snapshot().outbox_enqueued, 3);
    }

    #[test]
    fn total_conflicts_sums_the_five_resolution_paths() {
        let m = SyncMetrics::new();
        m.inc_conflict_lww_local_won();
        m.inc_conflict_lww_local_won();
        m.inc_conflict_lww_remote_won();
        m.inc_conflict_automerge();
        m.inc_conflict_server_transition();
        m.inc_conflict_user_action();
        let s = m.snapshot();
        assert_eq!(s.total_conflicts(), 6);
        // Each path's count is also independently visible —
        // the dashboard wants both the per-path breakdown
        // and the total.
        assert_eq!(s.conflicts_resolved_lww, 2);
        assert_eq!(s.conflicts_resolved_remote_won, 1);
        assert_eq!(s.conflicts_automerge, 1);
        assert_eq!(s.conflicts_server_transition, 1);
        assert_eq!(s.conflicts_user_action_required, 1);
    }

    #[test]
    fn pushing_in_distress_predicate_handles_no_attempts() {
        // Edge case: a freshly-booted client with no sync
        // activity is NOT in distress (the predicate must
        // not divide by zero or return true on n=0).
        let m = SyncMetrics::new();
        assert!(!m.snapshot().is_pushing_in_distress());
    }

    #[test]
    fn pushing_in_distress_predicate_trips_when_failures_outpace_successes() {
        let m = SyncMetrics::new();
        m.add_outbox_pushed(1);
        for _ in 0..15 {
            m.inc_outbox_push_failures();
        }
        // 15 failures against 1 success → 15 > max(1*10, 10)
        // = 10, so distress is true.
        assert!(m.snapshot().is_pushing_in_distress());
    }

    #[test]
    fn pushing_in_distress_predicate_stays_quiet_under_normal_load() {
        // 100 successes + 5 failures is normal-ish — every
        // production system has some transient failures, the
        // predicate should not trip until they dominate.
        let m = SyncMetrics::new();
        m.add_outbox_pushed(100);
        for _ in 0..5 {
            m.inc_outbox_push_failures();
        }
        assert!(!m.snapshot().is_pushing_in_distress());
    }

    #[test]
    fn outbox_backlog_saturates_at_zero() {
        // Defensive — should never happen in practice
        // (pushed monotone <= enqueued), but the saturating_sub
        // means a corrupted counter pair doesn't underflow
        // into a huge u64.
        let m = SyncMetrics::new();
        m.add_outbox_pushed(50);
        let s = m.snapshot();
        assert_eq!(s.outbox_backlog(), 0);
    }

    #[test]
    fn snapshot_round_trips_through_serde() {
        // Pin the wire format — the desktop renders this
        // JSON, so a field rename here breaks the dashboard.
        let m = SyncMetrics::new();
        m.inc_outbox_enqueued();
        m.add_outbox_pushed(1);
        m.inc_conflict_automerge();
        let s = m.snapshot();
        let json = serde_json::to_string(&s).unwrap();
        let back: MetricsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn concurrent_increments_from_many_threads_land_in_the_same_counter() {
        // The whole point of atomics — N threads each
        // increment M times should produce exactly N*M.
        use std::thread;
        let m = SyncMetrics::new();
        let mut handles = vec![];
        for _ in 0..8 {
            let m_clone = m.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    m_clone.inc_outbox_enqueued();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(m.snapshot().outbox_enqueued, 8 * 1000);
    }
}
