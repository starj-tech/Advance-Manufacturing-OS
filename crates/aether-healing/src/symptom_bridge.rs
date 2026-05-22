//! Bridges from upstream-crate symptom shapes to our local
//! `Symptom`.
//!
//! ## What this closes
//! V40 (`aether-sync::metrics_symptom`) and V41
//! (`aether-telemetry::circuit_symptom`) ship Symptom-shaped
//! values from their respective crates. Their docstrings
//! promise a one-line `From` impl on the healing side; this
//! module is that impl.
//!
//! Both bridges are feature-gated so a slim healing-only
//! deployment (no sync, no telemetry) doesn't pull in those
//! crates. Default features enable both since the typical
//! desktop binary uses every layer.
//!
//! ## Conversion contract
//! The upstream `MetricsSymptom` / `BreakerSymptom` already
//! carry the four fields `Symptom` needs (source, kind,
//! detail, severity). The conversion is a field-for-field
//! copy — pinning the wire-format equivalence is the whole
//! point of the contract.

// Only imported when at least one bridge feature is enabled.
// With every feature disabled this module compiles to
// nothing — the slim healing-only deployment pays no cost.
#[cfg(any(feature = "sync-bridge", feature = "telemetry-bridge"))]
use crate::diagnosis::Symptom;

#[cfg(feature = "sync-bridge")]
impl From<aether_sync::MetricsSymptom> for Symptom {
    fn from(src: aether_sync::MetricsSymptom) -> Self {
        // Field-for-field — the upstream struct was
        // intentionally laid out to match. If the shapes
        // ever diverge (e.g. healing adds a new field), the
        // compiler points us here.
        Self {
            source: src.source,
            kind: src.kind,
            detail: src.detail,
            severity: src.severity,
        }
    }
}

#[cfg(feature = "telemetry-bridge")]
impl From<aether_telemetry::BreakerSymptom> for Symptom {
    fn from(src: aether_telemetry::BreakerSymptom) -> Self {
        Self {
            source: src.source,
            kind: src.kind,
            detail: src.detail,
            severity: src.severity,
        }
    }
}

#[cfg(all(test, feature = "sync-bridge"))]
mod sync_bridge_tests {
    use super::*;
    use aether_sync::metrics_symptom::{metrics_symptoms, KIND_BACKLOG_HIGH, KIND_PUSH_DISTRESS};
    use aether_sync::SyncMetrics;

    #[test]
    fn metrics_symptom_converts_to_healing_symptom() {
        // Drive a real metrics snapshot to exercise the
        // whole V40 → conversion path, not just construct
        // a hand-rolled MetricsSymptom.
        let metrics = SyncMetrics::new();
        for _ in 0..1_500 {
            metrics.inc_outbox_enqueued();
        }
        let upstream = metrics_symptoms(&metrics.snapshot());
        assert!(!upstream.is_empty(), "expected at least one symptom");

        let converted: Vec<Symptom> = upstream.into_iter().map(Symptom::from).collect();
        // Every converted symptom retains its kind / severity
        // — pin so a future field rename in either crate
        // surfaces here.
        for s in &converted {
            assert!(!s.kind.is_empty());
            assert!(s.severity <= 3);
        }
        assert!(converted.iter().any(|s| s.kind == KIND_BACKLOG_HIGH));
    }

    #[test]
    fn high_severity_push_distress_round_trips_through_from_impl() {
        // Construct a known-severity scenario (push distress
        // = severity 2) and verify the converted Symptom
        // preserves it.
        let metrics = SyncMetrics::new();
        metrics.add_outbox_pushed(1);
        for _ in 0..15 {
            metrics.inc_outbox_push_failures();
        }
        let upstream = metrics_symptoms(&metrics.snapshot());
        let push_symptom = upstream
            .into_iter()
            .find(|s| s.kind == KIND_PUSH_DISTRESS)
            .expect("push_distress should be in the stream");
        let converted: Symptom = push_symptom.into();
        assert_eq!(converted.severity, 2);
        assert_eq!(converted.source, "aether-sync.metrics");
    }

    #[test]
    fn detail_payload_survives_conversion_intact() {
        // The dispatcher's diagnose() consumes the detail
        // JSON. Pin that the conversion doesn't drop / mangle
        // it.
        let metrics = SyncMetrics::new();
        for _ in 0..(aether_sync::BACKLOG_CRITICAL_WATERMARK + 5) {
            metrics.inc_outbox_enqueued();
        }
        let upstream = metrics_symptoms(&metrics.snapshot());
        let critical = upstream
            .into_iter()
            .find(|s| s.kind == aether_sync::metrics_symptom::KIND_BACKLOG_CRITICAL)
            .expect("backlog_critical present");
        let detail = critical.detail.clone();
        let converted: Symptom = critical.into();
        assert_eq!(converted.detail, detail);
        // Specific keys still navigable post-conversion.
        assert!(converted.detail.get("outbox_backlog").is_some());
        assert!(converted.detail.get("watermark").is_some());
    }
}

#[cfg(all(test, feature = "telemetry-bridge"))]
mod telemetry_bridge_tests {
    use super::*;
    use aether_telemetry::circuit_symptom::{breaker_symptom, KIND_BREAKER_OPEN};
    use aether_telemetry::{BreakerConfig, CircuitBreaker};
    use std::time::Duration;

    #[test]
    fn breaker_symptom_converts_to_healing_symptom() {
        // Trip a breaker for real, then run the V41 emitter,
        // then convert. Exercise the whole chain.
        let breaker = CircuitBreaker::with_config(BreakerConfig {
            failure_threshold: 2,
            cooldown: Duration::from_millis(50),
        });
        breaker.record_failure();
        breaker.record_failure();
        let upstream = breaker_symptom(&breaker, "sqlite-outbox").expect("open breaker emits");
        assert_eq!(upstream.kind, KIND_BREAKER_OPEN);

        let converted: Symptom = upstream.into();
        // Severity preserved (critical = 3 for breaker_open).
        assert_eq!(converted.severity, 3);
        // Source carries the sink name (per-sink routing
        // contract from V41).
        assert!(converted.source.contains("sqlite-outbox"));
    }

    #[test]
    fn closed_breaker_emits_nothing_so_no_conversion_happens() {
        // Defensive — if the upstream emitter returns None,
        // the conversion is never invoked. Pin the steady-
        // state quiet so a regression that wakes the
        // dispatcher on closed breakers is caught here.
        let breaker = CircuitBreaker::new();
        assert!(breaker_symptom(&breaker, "test").is_none());
    }
}
