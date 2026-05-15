//! `SyncStuckHealer` — the first concrete `Healer` in the Auto-Healing
//! moat. Recognizes "outbox isn't draining" and "pull is lagging"
//! symptoms and recommends a `RestartService { name: "sync" }`.
//!
//! ## Signal sources
//! Two `Symptom::kind` strings are owned by this healer:
//!
//! - `sync.outbox.stuck` — the local outbox depth has crossed a
//!   pre-configured threshold. Emitted by `aether-sync::engine`
//!   when `Outbox::size()` exceeds the watermark.
//! - `sync.pull.lag` — the time since the last successful
//!   `pull_since` exceeds the staleness budget. Emitted by the
//!   sync engine's background loop when its watchdog fires.
//!
//! Both symptoms carry their evidence in `detail` as JSON:
//!
//! ```json
//! { "depth": 1250, "lag_ms": 600000 }
//! ```
//!
//! Both fields are optional — a `depth`-only symptom (outbox stuck
//! but pull working) still scores, just lower than the both-firing
//! case.
//!
//! ## Confidence model
//! Each evidence field contributes up to 0.5; they're added and
//! capped at 1.0. Below `MIN_CONFIDENCE` the dispatcher ignores the
//! diagnosis anyway, so a thin signal (no detail at all) intentionally
//! falls under the floor.
//!
//! ## Apply
//! Returns a descriptor string. The actual subsystem-restart effect
//! lives at the desktop-app boundary (PR #5 wiring) — the healer's
//! responsibility ends with the policy and the audit-trail
//! description. Production swaps in a Tauri-channel-bound impl
//! that signals the running sync engine to bounce.

use crate::diagnosis::{Diagnosis, Healer, HealerError, Symptom};
use crate::policy::HealingPolicy;
use async_trait::async_trait;

/// Outbox depth at which we start contributing positive confidence.
/// Below this the queue is considered healthy backpressure, not stuck.
const DEPTH_NOISE_FLOOR: u64 = 100;
const DEPTH_HIGH: u64 = 500;
const DEPTH_CRITICAL: u64 = 1000;

/// Pull lag thresholds (ms). 10 s is "tolerable on a flaky link",
/// 1 min is "the engine is doing nothing useful", 5 min is "the
/// device is disconnected and we should warn loudly".
const LAG_NOISE_FLOOR_MS: u64 = 10_000;
const LAG_HIGH_MS: u64 = 60_000;
const LAG_CRITICAL_MS: u64 = 300_000;

pub struct SyncStuckHealer;

impl SyncStuckHealer {
    pub fn new() -> Self {
        Self
    }

    /// Score the outbox-depth evidence on a 0.0..=0.5 scale.
    fn score_depth(depth: u64) -> f32 {
        if depth >= DEPTH_CRITICAL {
            0.5
        } else if depth >= DEPTH_HIGH {
            0.3
        } else if depth >= DEPTH_NOISE_FLOOR {
            0.1
        } else {
            0.0
        }
    }

    /// Score the pull-lag evidence on a 0.0..=0.5 scale.
    fn score_lag(lag_ms: u64) -> f32 {
        if lag_ms >= LAG_CRITICAL_MS {
            0.5
        } else if lag_ms >= LAG_HIGH_MS {
            0.3
        } else if lag_ms >= LAG_NOISE_FLOOR_MS {
            0.1
        } else {
            0.0
        }
    }
}

impl Default for SyncStuckHealer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Healer for SyncStuckHealer {
    async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
        if !matches!(symptom.kind.as_str(), "sync.outbox.stuck" | "sync.pull.lag") {
            return Ok(None);
        }
        let depth = symptom
            .detail
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let lag_ms = symptom
            .detail
            .get("lag_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let confidence = (Self::score_depth(depth) + Self::score_lag(lag_ms)).min(1.0);

        // Confidence of exactly 0.0 means we recognized the kind but
        // saw no actual evidence — surface as None so the dispatcher
        // records "inconclusive" rather than auto-restarting on a
        // false alarm.
        if confidence == 0.0 {
            return Ok(None);
        }

        Ok(Some(Diagnosis {
            symptom: symptom.clone(),
            rationale: format!(
                "sync queue depth={depth} (high={DEPTH_HIGH}, critical={DEPTH_CRITICAL}); \
                 pull lag={lag_ms}ms (high={LAG_HIGH_MS}, critical={LAG_CRITICAL_MS})"
            ),
            confidence,
            recommended: HealingPolicy::RestartService {
                name: "sync".into(),
            },
        }))
    }

    async fn apply(&self, policy: &HealingPolicy) -> Result<String, HealerError> {
        match policy {
            HealingPolicy::RestartService { name } if name == "sync" => {
                // Production-wire: this is where the healer would
                // signal a tokio mpsc to the sync engine task,
                // requesting a clean bounce. Test scaffold returns
                // a descriptor — the dispatcher records it to the
                // ledger so operators see the action even when the
                // production effect channel is mocked out.
                Ok("requested clean restart of sync subsystem".into())
            }
            other => Err(HealerError::NotImplemented(match other {
                HealingPolicy::RestartService { .. } => "RestartService for non-sync target",
                HealingPolicy::RollbackModule { .. } => "RollbackModule outside healer scope",
                HealingPolicy::RecoverSqlite { .. } => "RecoverSqlite outside healer scope",
                HealingPolicy::TrimTelemetry { .. } => "TrimTelemetry outside healer scope",
                HealingPolicy::EscalateToHuman { .. } => "EscalateToHuman outside healer scope",
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn symptom(kind: &str, detail: serde_json::Value) -> Symptom {
        Symptom {
            source: "aether-sync".into(),
            kind: kind.into(),
            detail,
            severity: 2,
        }
    }

    #[tokio::test]
    async fn ignores_unrelated_kinds() {
        let h = SyncStuckHealer::new();
        assert!(h
            .diagnose(&symptom("opcua.disconnect", json!({})))
            .await
            .unwrap()
            .is_none());
        assert!(h
            .diagnose(&symptom("module.crash", json!({"id": "x"})))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn recognizes_outbox_stuck_at_critical_depth() {
        let h = SyncStuckHealer::new();
        let d = h
            .diagnose(&symptom(
                "sync.outbox.stuck",
                json!({ "depth": 1500, "lag_ms": 0 }),
            ))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(d.confidence >= 0.5, "critical depth scores >= 0.5");
        assert!(matches!(
            d.recommended,
            HealingPolicy::RestartService { ref name } if name == "sync"
        ));
    }

    #[tokio::test]
    async fn recognizes_pull_lag_at_critical_threshold() {
        let h = SyncStuckHealer::new();
        let d = h
            .diagnose(&symptom("sync.pull.lag", json!({ "lag_ms": 600_000 })))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(d.confidence >= 0.5);
    }

    #[tokio::test]
    async fn both_signals_combine_to_higher_confidence() {
        // depth=1500 (critical, 0.5) + lag=600s (critical, 0.5) = 1.0
        // depth=600 (high, 0.3) + lag=120s (high, 0.3) = 0.6
        let h = SyncStuckHealer::new();
        let both_critical = h
            .diagnose(&symptom(
                "sync.outbox.stuck",
                json!({ "depth": 1500, "lag_ms": 600_000 }),
            ))
            .await
            .unwrap()
            .unwrap();
        let both_high = h
            .diagnose(&symptom(
                "sync.outbox.stuck",
                json!({ "depth": 600, "lag_ms": 120_000 }),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(both_critical.confidence > both_high.confidence);
        assert!((both_critical.confidence - 1.0).abs() < 1e-3);
        assert!((both_high.confidence - 0.6).abs() < 1e-3);
    }

    #[tokio::test]
    async fn confidence_saturates_at_one() {
        // Even if both scores combined exceeded 1.0, we don't want a
        // dispatcher comparing 1.2 vs 0.9 — confidences are a
        // probability-ish scale and shouldn't break the invariant.
        let h = SyncStuckHealer::new();
        let d = h
            .diagnose(&symptom(
                "sync.outbox.stuck",
                json!({ "depth": u64::MAX, "lag_ms": u64::MAX }),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(d.confidence <= 1.0);
    }

    #[tokio::test]
    async fn no_evidence_returns_none_not_low_confidence() {
        // A symptom that only has the kind but no detail body shouldn't
        // trigger an auto-restart. The healer surfaces None so the
        // dispatcher writes "inconclusive" rather than "applied".
        let h = SyncStuckHealer::new();
        let d = h
            .diagnose(&symptom("sync.outbox.stuck", json!({})))
            .await
            .unwrap();
        assert!(d.is_none(), "no evidence → no diagnosis");
    }

    #[tokio::test]
    async fn noise_floor_depth_does_not_trigger_alone() {
        // A small queue depth (just into the noise floor) on its own
        // — no lag — yields only 0.1 confidence. Above the dispatcher
        // floor but barely. The diagnosis is still returned; the
        // dispatcher decides whether to apply.
        let h = SyncStuckHealer::new();
        let d = h
            .diagnose(&symptom("sync.outbox.stuck", json!({ "depth": 150 })))
            .await
            .unwrap()
            .unwrap();
        assert!((d.confidence - 0.1).abs() < 1e-3);
    }

    #[tokio::test]
    async fn apply_returns_descriptor_for_sync_restart() {
        let h = SyncStuckHealer::new();
        let desc = h
            .apply(&HealingPolicy::RestartService {
                name: "sync".into(),
            })
            .await
            .unwrap();
        assert!(
            desc.contains("sync"),
            "descriptor mentions the affected subsystem: {desc}"
        );
    }

    #[tokio::test]
    async fn apply_rejects_unrelated_policies() {
        let h = SyncStuckHealer::new();
        assert!(matches!(
            h.apply(&HealingPolicy::RestartService {
                name: "opcua".into()
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
        assert!(matches!(
            h.apply(&HealingPolicy::TrimTelemetry {
                keep_recent_hours: 24
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
    }
}
