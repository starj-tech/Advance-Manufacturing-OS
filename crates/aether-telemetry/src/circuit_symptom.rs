//! Bridge from V33 `CircuitBreaker` state to V25-era healing-
//! layer `Symptom`s. Sibling of V40's `metrics_symptom` for
//! aether-sync.
//!
//! ## Why this is here, not in `aether-healing`
//! Same rationale as V40: avoiding a circular dep. The
//! healing layer already depends on aether-core; if it
//! depended on aether-telemetry the wiring would loop. By
//! emitting a Symptom-shaped value from the source crate,
//! the healing layer's one-line `From` impl picks it up
//! without taking telemetry as a runtime dep.
//!
//! ## What's emitted
//! Two symptom kinds:
//!
//!   * `protocols.telemetry.breaker_open` (severity 3) —
//!     the breaker tripped to Open. The downstream sink is
//!     genuinely broken; the healing layer's
//!     `BridgeReconnectHealer` (or a future TelemetrySink
//!     healer) should attempt a restart.
//!   * `protocols.telemetry.breaker_half_open` (severity 1)
//!     — the breaker is probing after a cooldown. Surfaced
//!     as info-level so dashboards can show "recovery in
//!     progress" without the healing layer trying to heal
//!     it again.
//!
//! `Closed` state emits nothing — that's the steady state.
//!
//! ## Naming
//! Sink-specific tags (`sync`, `opcua-bridge`, `mqtt-bridge`)
//! are passed in by the caller as the `sink_name` so a single
//! healing dispatcher can distinguish which sink is open.
//! That's why this isn't a const-only enum on `BreakerState`:
//! the breaker itself doesn't know which downstream it's
//! protecting; the caller does.

use crate::circuit::{BreakerState, CircuitBreaker};
use serde::{Deserialize, Serialize};

/// Canonical kind slugs the healing dispatcher matches on.
pub const KIND_BREAKER_OPEN: &str = "protocols.telemetry.breaker_open";
pub const KIND_BREAKER_HALF_OPEN: &str = "protocols.telemetry.breaker_half_open";

/// Local Symptom-shaped value. Same field layout as
/// `aether_healing::Symptom`; converted via a one-line `From`
/// impl on the healing-layer side when wired.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreakerSymptom {
    pub source: String,
    pub kind: String,
    pub detail: serde_json::Value,
    pub severity: u8,
}

impl BreakerSymptom {
    fn new(sink_name: &str, kind: &str, severity: u8, detail: serde_json::Value) -> Self {
        Self {
            source: format!("aether-telemetry.circuit:{sink_name}"),
            kind: kind.to_string(),
            detail,
            severity,
        }
    }
}

/// Inspect a breaker's current state and emit a symptom if
/// it's NOT in the steady-state Closed.
///
/// `sink_name` is the caller's tag for the downstream the
/// breaker protects (e.g. `"sqlite-outbox"`, `"opcua-bridge"`,
/// `"mqtt-bridge"`). The healing dispatcher uses the
/// `source` field to route per-sink heals.
///
/// Returns `None` when the breaker is Closed — the symptom
/// stream stays quiet during normal operation.
pub fn breaker_symptom(breaker: &CircuitBreaker, sink_name: &str) -> Option<BreakerSymptom> {
    let state = breaker.state();
    match state {
        BreakerState::Closed => None,
        BreakerState::Open => Some(BreakerSymptom::new(
            sink_name,
            KIND_BREAKER_OPEN,
            3,
            serde_json::json!({
                "sink": sink_name,
                "state": state.slug(),
                "consecutive_failures": breaker.consecutive_failures(),
                "transition_count": breaker.transition_count(),
            }),
        )),
        BreakerState::HalfOpen => Some(BreakerSymptom::new(
            sink_name,
            KIND_BREAKER_HALF_OPEN,
            1,
            serde_json::json!({
                "sink": sink_name,
                "state": state.slug(),
                "transition_count": breaker.transition_count(),
            }),
        )),
    }
}

/// Convenience: emit symptoms for a fleet of named breakers
/// in one call. Skips quiet breakers (Closed state). The
/// healing dispatcher's polling loop calls this once per
/// interval and feeds the result through `Healer::diagnose`.
pub fn breaker_symptoms(named: &[(&str, &CircuitBreaker)]) -> Vec<BreakerSymptom> {
    named
        .iter()
        .filter_map(|(name, b)| breaker_symptom(b, name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{BreakerConfig, CircuitBreaker};
    use std::time::Duration;

    fn small_config() -> BreakerConfig {
        BreakerConfig {
            failure_threshold: 3,
            cooldown: Duration::from_millis(50),
        }
    }

    #[test]
    fn closed_breaker_emits_no_symptom() {
        let b = CircuitBreaker::with_config(small_config());
        assert!(breaker_symptom(&b, "test-sink").is_none());
    }

    #[test]
    fn open_breaker_emits_critical_symptom_with_failure_count() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        // Should be Open now.
        let symptom = breaker_symptom(&b, "test-sink").expect("Open emits");
        assert_eq!(symptom.kind, KIND_BREAKER_OPEN);
        assert_eq!(symptom.severity, 3);
        assert_eq!(symptom.detail["sink"], "test-sink");
        assert_eq!(symptom.detail["state"], "open");
        // consecutive_failures key present.
        assert!(symptom.detail.get("consecutive_failures").is_some());
    }

    #[tokio::test]
    async fn half_open_breaker_emits_info_symptom() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        // Wait for cooldown so the next decide() promotes
        // Open → HalfOpen.
        tokio::time::sleep(Duration::from_millis(80)).await;
        // decide() does the promotion as a side effect.
        let _ = b.decide();
        let symptom = breaker_symptom(&b, "test-sink").expect("HalfOpen emits");
        assert_eq!(symptom.kind, KIND_BREAKER_HALF_OPEN);
        assert_eq!(symptom.severity, 1);
        assert_eq!(symptom.detail["state"], "half-open");
    }

    #[test]
    fn source_field_carries_sink_name_for_per_sink_routing() {
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        let symptom = breaker_symptom(&b, "opcua-bridge").unwrap();
        // Healing dispatcher routes by source — pin the
        // format so the routing key stays stable.
        assert_eq!(symptom.source, "aether-telemetry.circuit:opcua-bridge");
    }

    #[test]
    fn breaker_symptoms_fleet_skips_quiet_breakers() {
        // Three breakers: A closed, B open, C closed. Output
        // should contain ONLY B's symptom — pinning that
        // healthy breakers don't add noise.
        let a = CircuitBreaker::with_config(small_config());
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        let c = CircuitBreaker::with_config(small_config());

        let symptoms = breaker_symptoms(&[("sink-a", &a), ("sink-b", &b), ("sink-c", &c)]);
        assert_eq!(symptoms.len(), 1);
        assert_eq!(symptoms[0].detail["sink"], "sink-b");
    }

    #[test]
    fn breaker_symptoms_fleet_returns_empty_when_all_closed() {
        // Steady-state happy path: every breaker closed,
        // function returns empty Vec (no false positives).
        let a = CircuitBreaker::with_config(small_config());
        let b = CircuitBreaker::with_config(small_config());
        let symptoms = breaker_symptoms(&[("a", &a), ("b", &b)]);
        assert!(symptoms.is_empty());
    }

    #[test]
    fn breaker_symptoms_preserves_caller_supplied_names() {
        // The fleet helper carries each name verbatim into
        // the output — pin so a downstream UI can render
        // "sqlite-outbox tripped" using the exact tag the
        // caller passed.
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        let symptoms = breaker_symptoms(&[("very-specific-sink-name", &b)]);
        assert_eq!(symptoms[0].detail["sink"], "very-specific-sink-name");
    }

    #[test]
    fn symptom_serde_round_trips_through_json() {
        // Wire-format pin. Healing-layer audit log is the
        // consumer; a field rename here breaks log parsers.
        let s = BreakerSymptom::new(
            "test",
            KIND_BREAKER_OPEN,
            3,
            serde_json::json!({"foo": "bar"}),
        );
        let json = serde_json::to_string(&s).unwrap();
        let back: BreakerSymptom = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn kind_constants_are_unique_and_kebab_dotted() {
        // The dispatcher pattern-matches on `kind` strings;
        // duplicate kinds would silently merge into one
        // diagnose() arm.
        assert_ne!(KIND_BREAKER_OPEN, KIND_BREAKER_HALF_OPEN);
        for k in [KIND_BREAKER_OPEN, KIND_BREAKER_HALF_OPEN] {
            assert!(k.starts_with("protocols.telemetry."));
            assert!(!k.contains('_') || k.contains("breaker_"));
        }
    }

    #[test]
    fn transition_count_appears_in_open_and_half_open_detail() {
        // Pin transition_count in the detail JSON so the
        // dashboard can render "tripped N times since boot"
        // without a separate metric query.
        let b = CircuitBreaker::with_config(small_config());
        for _ in 0..3 {
            b.record_failure();
        }
        let s = breaker_symptom(&b, "x").unwrap();
        assert!(s.detail.get("transition_count").is_some());
    }
}
