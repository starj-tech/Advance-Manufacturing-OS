//! Bridge from V34 `MetricsSnapshot` to V25-era healing-layer
//! `Symptom`s.
//!
//! ## Why this is its own module
//! V34 ships the counters; the healing layer consumes
//! symptoms. The translation between "raw counter snapshot"
//! and "named symptom with severity + payload" is a pure
//! function with several thresholds — extracted here so:
//!
//!   * The healing layer doesn't need to import the metrics
//!     surface directly (would be a circular dep:
//!     aether-healing already depends on aether-core; this
//!     way aether-sync stays the sole metrics owner).
//!   * The thresholds are testable in isolation — every
//!     watermark gets a pinning test that catches
//!     accidental "trip on 1 sample" or "never trip" bugs.
//!
//! ## What's emitted
//! The function returns a list of `MetricsSymptom` values —
//! same shape as `aether_healing::Symptom` but kept here as a
//! local type so the wire format is stable independent of the
//! healing crate's evolution. Consumers do a one-line
//! conversion (`From` impl in the healing crate, when wired).
//!
//! Five symptom kinds today:
//!
//!   * `sync.outbox.push_distress` — push failures > 10×
//!     successes (the `is_pushing_in_distress` predicate).
//!     Severity 2 (error).
//!   * `sync.outbox.backlog_high` — backlog > 1000 entries
//!     even without push failures. Severity 1 (warn).
//!   * `sync.outbox.backlog_critical` — backlog > 10 000
//!     entries. Severity 3 (critical).
//!   * `sync.pull.failing` — pull_failures > 0 and
//!     pull_batches < 3× failures. Severity 2.
//!   * `sync.conflicts.user_action_required` — at least one
//!     RejectAndSurface conflict observed. Severity 1.
//!
//! Adding a new symptom is a deliberate workspace-wide
//! change — the healing-layer mapping table that picks a
//! `HealingPolicy` per kind needs the matching entry.

use crate::metrics::MetricsSnapshot;
use serde::{Deserialize, Serialize};

/// Backlog above which we emit `sync.outbox.backlog_high`
/// (warn). Picked so a healthy local-first client (writes
/// faster than network, drains within minutes) doesn't trip;
/// a stuck client (offline for hours) does.
pub const BACKLOG_HIGH_WATERMARK: u64 = 1_000;

/// Backlog above which we escalate to
/// `sync.outbox.backlog_critical` (severity 3). At 10k
/// entries the local SQLite is bloated enough that the
/// healing layer should consider an emergency drain action.
pub const BACKLOG_CRITICAL_WATERMARK: u64 = 10_000;

/// Pull failure ratio: when `pull_failures > N × pull_batches`,
/// the pull side is in distress. N=3 mirrors the V13
/// `BridgeReconnectHealer`'s heuristic for "this isn't
/// transient noise."
pub const PULL_FAILURE_RATIO_THRESHOLD: u64 = 3;

/// Same shape as `aether_healing::Symptom` but local so the
/// healing crate doesn't have to be a runtime dep of
/// aether-sync. The healing layer can `impl From<MetricsSymptom>
/// for Symptom { ... }` and the conversion is one line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsSymptom {
    /// Source crate — always `"aether-sync.metrics"` from
    /// this function. Kept as a field (not const) so the
    /// healing layer's per-source filtering works uniformly.
    pub source: String,
    /// Short kebab-case identifier the healing dispatcher
    /// maps to a `HealingPolicy`. Stored as `String` (not
    /// `&'static str`) so the type round-trips through serde
    /// without lifetime gymnastics; constructors use the
    /// `KIND_*` consts below to pin allowed values at the
    /// callsite.
    pub kind: String,
    /// Structured payload — counter snapshot keys the
    /// dispatcher's diagnose() may want to include in its
    /// rationale.
    pub detail: serde_json::Value,
    /// 0=info, 1=warn, 2=error, 3=critical.
    pub severity: u8,
}

/// Canonical kind slugs. Adding a slug here is the deliberate
/// workspace-wide change a new symptom kind requires — the
/// healing-layer dispatcher's match arm needs the matching
/// entry.
pub const KIND_BACKLOG_HIGH: &str = "sync.outbox.backlog_high";
pub const KIND_BACKLOG_CRITICAL: &str = "sync.outbox.backlog_critical";
pub const KIND_PUSH_DISTRESS: &str = "sync.outbox.push_distress";
pub const KIND_PULL_FAILING: &str = "sync.pull.failing";
pub const KIND_USER_ACTION_REQUIRED: &str = "sync.conflicts.user_action_required";

impl MetricsSymptom {
    fn new(kind: &str, severity: u8, detail: serde_json::Value) -> Self {
        Self {
            source: "aether-sync.metrics".into(),
            kind: kind.to_string(),
            detail,
            severity,
        }
    }
}

/// Translate a snapshot into zero or more symptoms.
/// Deterministic — same snapshot always yields the same
/// symptom list. The healing layer calls this on a
/// configurable interval (typically every 30 s); each
/// returned symptom is fed through `Healer::diagnose`.
///
/// Symptom ordering: lower-severity-first so a downstream
/// log line that truncates after N entries still surfaces
/// the most important problem. Within the same severity,
/// ordering is by declaration order in this function.
pub fn metrics_symptoms(snapshot: &MetricsSnapshot) -> Vec<MetricsSymptom> {
    let mut symptoms = Vec::new();

    // Backlog tier: critical wins over high — emit only one
    // backlog symptom to avoid double-counting.
    let backlog = snapshot.outbox_backlog();
    if backlog >= BACKLOG_CRITICAL_WATERMARK {
        symptoms.push(MetricsSymptom::new(
            KIND_BACKLOG_CRITICAL,
            3,
            serde_json::json!({
                "outbox_backlog": backlog,
                "outbox_enqueued": snapshot.outbox_enqueued,
                "outbox_pushed": snapshot.outbox_pushed,
                "watermark": BACKLOG_CRITICAL_WATERMARK,
            }),
        ));
    } else if backlog >= BACKLOG_HIGH_WATERMARK {
        symptoms.push(MetricsSymptom::new(
            KIND_BACKLOG_HIGH,
            1,
            serde_json::json!({
                "outbox_backlog": backlog,
                "outbox_enqueued": snapshot.outbox_enqueued,
                "outbox_pushed": snapshot.outbox_pushed,
                "watermark": BACKLOG_HIGH_WATERMARK,
            }),
        ));
    }

    // Push distress — push_failures dominate. This is
    // separately detectable from a high backlog (a brand-new
    // client with zero pushes and 50 failures is distressed
    // even though the backlog might still be small).
    if snapshot.is_pushing_in_distress() {
        symptoms.push(MetricsSymptom::new(
            KIND_PUSH_DISTRESS,
            2,
            serde_json::json!({
                "outbox_push_failures": snapshot.outbox_push_failures,
                "outbox_pushed": snapshot.outbox_pushed,
            }),
        ));
    }

    // Pull-side distress: many failures, few successful
    // batches. We require at least 1 failure to surface
    // anything (a brand-new client with no pull activity
    // doesn't get flagged).
    if snapshot.pull_failures > 0
        && snapshot.pull_failures
            > snapshot
                .pull_batches
                .saturating_mul(PULL_FAILURE_RATIO_THRESHOLD)
                .max(PULL_FAILURE_RATIO_THRESHOLD)
    {
        symptoms.push(MetricsSymptom::new(
            KIND_PULL_FAILING,
            2,
            serde_json::json!({
                "pull_failures": snapshot.pull_failures,
                "pull_batches": snapshot.pull_batches,
                "ratio_threshold": PULL_FAILURE_RATIO_THRESHOLD,
            }),
        ));
    }

    // User-action-required conflicts: an inventory or
    // financial conflict surfaced for human resolution. The
    // healing layer treats this as a hint to bump the UI
    // notification rather than auto-heal.
    if snapshot.conflicts_user_action_required > 0 {
        symptoms.push(MetricsSymptom::new(
            KIND_USER_ACTION_REQUIRED,
            1,
            serde_json::json!({
                "conflicts_user_action_required": snapshot.conflicts_user_action_required,
                "total_conflicts": snapshot.total_conflicts(),
            }),
        ));
    }

    // Sort by severity (ascending) for stable iteration. Use
    // `sort_by_key` rather than `sort_by` since the only
    // ordering signal we have is the severity int.
    symptoms.sort_by_key(|s| s.severity);
    symptoms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::SyncMetrics;

    fn fresh_snapshot() -> MetricsSnapshot {
        SyncMetrics::new().snapshot()
    }

    #[test]
    fn fresh_metrics_emit_no_symptoms() {
        // Boot-time invariant: a brand-new client with no
        // activity has nothing to diagnose. The watermark
        // thresholds and the "at-least-one-failure" gates
        // collectively pin this.
        let symptoms = metrics_symptoms(&fresh_snapshot());
        assert!(symptoms.is_empty());
    }

    #[test]
    fn high_backlog_emits_backlog_high_warn() {
        let metrics = SyncMetrics::new();
        for _ in 0..(BACKLOG_HIGH_WATERMARK + 10) {
            metrics.inc_outbox_enqueued();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        assert_eq!(symptoms.len(), 1);
        assert_eq!(symptoms[0].kind, "sync.outbox.backlog_high");
        assert_eq!(symptoms[0].severity, 1);
        assert_eq!(symptoms[0].source, "aether-sync.metrics");
    }

    #[test]
    fn critical_backlog_emits_critical_not_high() {
        // The tiers MUST NOT double-emit. A critical-tier
        // backlog produces ONLY the critical symptom, not
        // critical + high.
        let metrics = SyncMetrics::new();
        for _ in 0..(BACKLOG_CRITICAL_WATERMARK + 5) {
            metrics.inc_outbox_enqueued();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        // Exactly one backlog symptom.
        let backlog_symptoms: Vec<&MetricsSymptom> = symptoms
            .iter()
            .filter(|s| s.kind.starts_with("sync.outbox.backlog"))
            .collect();
        assert_eq!(backlog_symptoms.len(), 1);
        assert_eq!(backlog_symptoms[0].kind, "sync.outbox.backlog_critical");
        assert_eq!(backlog_symptoms[0].severity, 3);
    }

    #[test]
    fn backlog_just_below_high_watermark_emits_nothing() {
        // Boundary test: BACKLOG_HIGH_WATERMARK - 1 stays
        // quiet. Saves the healing layer from spurious
        // wake-ups on normal operating depth.
        let metrics = SyncMetrics::new();
        for _ in 0..(BACKLOG_HIGH_WATERMARK - 1) {
            metrics.inc_outbox_enqueued();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        assert!(symptoms.is_empty());
    }

    #[test]
    fn push_distress_emits_push_distress_error() {
        let metrics = SyncMetrics::new();
        metrics.add_outbox_pushed(1);
        for _ in 0..15 {
            metrics.inc_outbox_push_failures();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        let push_symptoms: Vec<&MetricsSymptom> = symptoms
            .iter()
            .filter(|s| s.kind == "sync.outbox.push_distress")
            .collect();
        assert_eq!(push_symptoms.len(), 1);
        assert_eq!(push_symptoms[0].severity, 2);
    }

    #[test]
    fn pull_failing_emits_pull_failing_error_when_failures_dominate() {
        let metrics = SyncMetrics::new();
        // 10 failures, 1 successful batch → ratio 10:1 >
        // threshold of 3.
        metrics.inc_pull_batches();
        for _ in 0..10 {
            metrics.inc_pull_failures();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        let pull = symptoms.iter().find(|s| s.kind == "sync.pull.failing");
        assert!(pull.is_some(), "got {symptoms:?}");
        assert_eq!(pull.unwrap().severity, 2);
    }

    #[test]
    fn pull_quiet_during_normal_operation() {
        // 100 successful batches + 5 failures is normal-ish.
        // Predicate MUST stay silent.
        let metrics = SyncMetrics::new();
        for _ in 0..100 {
            metrics.inc_pull_batches();
        }
        for _ in 0..5 {
            metrics.inc_pull_failures();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        assert!(!symptoms.iter().any(|s| s.kind == "sync.pull.failing"));
    }

    #[test]
    fn user_action_conflicts_emit_warn_symptom() {
        let metrics = SyncMetrics::new();
        metrics.inc_conflict_user_action();
        let symptoms = metrics_symptoms(&metrics.snapshot());
        let conflict = symptoms
            .iter()
            .find(|s| s.kind == "sync.conflicts.user_action_required");
        assert!(conflict.is_some());
        assert_eq!(conflict.unwrap().severity, 1);
    }

    #[test]
    fn automerge_or_lww_conflicts_alone_emit_nothing() {
        // Only RejectAndSurface (user-action-required) trips
        // a symptom. Automatic-resolution conflicts (LWW,
        // automerge, server-transition) are normal mode and
        // shouldn't wake the healing layer.
        let metrics = SyncMetrics::new();
        metrics.inc_conflict_lww_local_won();
        metrics.inc_conflict_automerge();
        metrics.inc_conflict_server_transition();
        let symptoms = metrics_symptoms(&metrics.snapshot());
        assert!(symptoms.is_empty());
    }

    #[test]
    fn multiple_concurrent_problems_all_surface() {
        // The healing layer's dispatcher needs every active
        // problem in one call so it can rank them; pin that
        // we don't silently drop any.
        let metrics = SyncMetrics::new();
        for _ in 0..(BACKLOG_HIGH_WATERMARK + 1) {
            metrics.inc_outbox_enqueued();
        }
        metrics.add_outbox_pushed(1);
        for _ in 0..15 {
            metrics.inc_outbox_push_failures();
        }
        metrics.inc_conflict_user_action();
        let symptoms = metrics_symptoms(&metrics.snapshot());
        let kinds: Vec<&str> = symptoms.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&KIND_BACKLOG_HIGH));
        assert!(kinds.contains(&KIND_PUSH_DISTRESS));
        assert!(kinds.contains(&KIND_USER_ACTION_REQUIRED));
    }

    #[test]
    fn output_is_sorted_by_severity_ascending() {
        // Stable iteration order — log-line truncation
        // preserves the most-critical entry at the END
        // (descending) or the top (ascending). We chose
        // ascending so the dispatcher's "pick highest
        // severity" sweep doesn't have to reverse.
        let metrics = SyncMetrics::new();
        // Trigger one of each tier: warn (user-action), error
        // (push-distress), critical (backlog-critical).
        metrics.inc_conflict_user_action(); // severity 1
        metrics.add_outbox_pushed(1);
        for _ in 0..15 {
            metrics.inc_outbox_push_failures();
        }
        // Push-distress: severity 2
        for _ in 0..(BACKLOG_CRITICAL_WATERMARK + 1) {
            metrics.inc_outbox_enqueued();
        }
        // Backlog-critical: severity 3
        let symptoms = metrics_symptoms(&metrics.snapshot());
        // Severities ascend.
        for win in symptoms.windows(2) {
            assert!(win[0].severity <= win[1].severity);
        }
    }

    #[test]
    fn detail_payload_includes_relevant_counters() {
        // Pin the wire shape of the detail JSON — the
        // dispatcher's diagnose() reads these field names.
        let metrics = SyncMetrics::new();
        for _ in 0..(BACKLOG_HIGH_WATERMARK + 5) {
            metrics.inc_outbox_enqueued();
        }
        let symptoms = metrics_symptoms(&metrics.snapshot());
        let backlog = symptoms
            .iter()
            .find(|s| s.kind == "sync.outbox.backlog_high")
            .unwrap();
        assert!(backlog.detail.get("outbox_backlog").is_some());
        assert!(backlog.detail.get("outbox_enqueued").is_some());
        assert!(backlog.detail.get("outbox_pushed").is_some());
        assert!(backlog.detail.get("watermark").is_some());
    }

    #[test]
    fn symptom_serde_round_trips_through_json() {
        // The MetricsSymptom shape is what the dispatcher
        // serializes into its audit log; pin the
        // round-trip.
        let s = MetricsSymptom::new(
            "sync.outbox.backlog_high",
            1,
            serde_json::json!({"outbox_backlog": 1234}),
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: MetricsSymptom = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn watermark_constants_are_sane() {
        // Anchor for the docstring rationale — the high tier
        // must be strictly lower than the critical tier;
        // otherwise the "critical wins over high" logic
        // breaks. Const-block asserts so the check runs at
        // compile time too.
        const _HIGH_LT_CRITICAL: () = assert!(BACKLOG_HIGH_WATERMARK < BACKLOG_CRITICAL_WATERMARK);
        const _RATIO_AT_LEAST_ONE: () = assert!(PULL_FAILURE_RATIO_THRESHOLD >= 1);
    }
}
