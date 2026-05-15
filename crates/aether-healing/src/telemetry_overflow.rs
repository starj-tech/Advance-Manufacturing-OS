//! `TelemetryOverflowHealer` — recognize when the telemetry buffer is
//! growing past safe bounds (or the disk holding it is filling) and
//! recommend trimming old samples.
//!
//! ## Signal sources
//! - `telemetry.buffer.overflow` — in-memory ring buffer in
//!   `aether-telemetry` is approaching or exceeding capacity.
//! - `telemetry.disk.pressure` — SQLite telemetry hypertable is
//!   eating disk faster than retention is freeing it.
//!
//! Evidence carried in `detail`:
//!
//! ```json
//! {
//!   "buffer_depth": 50000,
//!   "buffer_capacity": 100000,
//!   "disk_free_mb": 250,
//!   "samples_dropped": 1240
//! }
//! ```
//!
//! ## Destructive policy gating
//! This healer's diagnosis is always a `HealingPolicy::TrimTelemetry`,
//! which `HealingPolicy::is_destructive() == true`. The dispatcher
//! refuses to auto-apply it on the client — the trim runs only after
//! an Edge Function with operator confirmation says go. That keeps
//! the audit trail clean: an operator can always answer "who
//! decided to drop that 4-hour window of telemetry?".

use crate::diagnosis::{Diagnosis, Healer, HealerError, Symptom};
use crate::policy::HealingPolicy;
use async_trait::async_trait;

/// Fraction-of-capacity thresholds for the in-memory buffer.
/// 0.75 = "starting to fill", 0.90 = "actively dropping samples
/// soon", 0.98 = "any further write will overflow".
const BUF_NOISE_FLOOR: f64 = 0.75;
const BUF_HIGH: f64 = 0.90;
const BUF_CRITICAL: f64 = 0.98;

/// Disk-free thresholds (MB). 500 MB is "watch this", 200 MB is
/// "act soon", 100 MB is "act now". Reading goes "lower is worse".
const DISK_NOISE_FLOOR_MB: u64 = 500;
const DISK_HIGH_MB: u64 = 200;
const DISK_CRITICAL_MB: u64 = 100;

/// Default trim window when this healer fires. 24h gives us a full
/// shift's history while reclaiming the majority of accumulated bytes
/// — operators can pull the surface anomaly into a longer retention
/// tier explicitly if needed.
const DEFAULT_KEEP_RECENT_HOURS: u32 = 24;

pub struct TelemetryOverflowHealer;

impl TelemetryOverflowHealer {
    pub fn new() -> Self {
        Self
    }

    /// Score the in-memory buffer pressure on a 0.0..=0.5 scale.
    /// Takes `depth` and `capacity` separately so a zero-capacity
    /// telemetry config (unlikely but possible in tests) reports
    /// 0.0 rather than dividing by zero.
    fn score_buffer(depth: u64, capacity: u64) -> f32 {
        if capacity == 0 {
            return 0.0;
        }
        let ratio = depth as f64 / capacity as f64;
        if ratio >= BUF_CRITICAL {
            0.5
        } else if ratio >= BUF_HIGH {
            0.3
        } else if ratio >= BUF_NOISE_FLOOR {
            0.1
        } else {
            0.0
        }
    }

    /// Score the disk-pressure evidence on a 0.0..=0.5 scale.
    /// Lower free space = higher score (inverse of the buffer
    /// threshold direction).
    fn score_disk(disk_free_mb: u64) -> f32 {
        if disk_free_mb <= DISK_CRITICAL_MB {
            0.5
        } else if disk_free_mb <= DISK_HIGH_MB {
            0.3
        } else if disk_free_mb <= DISK_NOISE_FLOOR_MB {
            0.1
        } else {
            0.0
        }
    }
}

impl Default for TelemetryOverflowHealer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Healer for TelemetryOverflowHealer {
    async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
        if !matches!(
            symptom.kind.as_str(),
            "telemetry.buffer.overflow" | "telemetry.disk.pressure"
        ) {
            return Ok(None);
        }
        let depth = symptom
            .detail
            .get("buffer_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let capacity = symptom
            .detail
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        // Disk reports the inverse direction. Default to "lots free"
        // rather than 0 so a missing field doesn't accidentally
        // trigger a disk-pressure alarm.
        let disk_free_mb = symptom
            .detail
            .get("disk_free_mb")
            .and_then(|v| v.as_u64())
            .unwrap_or(u64::MAX);

        let confidence =
            (Self::score_buffer(depth, capacity) + Self::score_disk(disk_free_mb)).min(1.0);

        if confidence == 0.0 {
            return Ok(None);
        }

        Ok(Some(Diagnosis {
            symptom: symptom.clone(),
            rationale: format!(
                "telemetry buffer {depth}/{capacity}; disk_free={disk_free_mb}MB \
                 (critical<={DISK_CRITICAL_MB}MB)"
            ),
            confidence,
            recommended: HealingPolicy::TrimTelemetry {
                keep_recent_hours: DEFAULT_KEEP_RECENT_HOURS,
            },
        }))
    }

    async fn apply(&self, policy: &HealingPolicy) -> Result<String, HealerError> {
        match policy {
            HealingPolicy::TrimTelemetry { keep_recent_hours } => {
                // Production: invoked only via cloud-confirmed Edge
                // Function path (dispatcher refuses to auto-apply on
                // the client). Performs `DELETE FROM telemetry_samples
                // WHERE ts < now() - $keep_recent_hours` via aether-db.
                // Skeleton returns the descriptor.
                Ok(format!(
                    "would trim telemetry to last {keep_recent_hours}h \
                     (requires cloud-confirmation path)"
                ))
            }
            HealingPolicy::RestartService { .. } => Err(HealerError::NotImplemented(
                "RestartService outside healer scope",
            )),
            HealingPolicy::RollbackModule { .. } => Err(HealerError::NotImplemented(
                "RollbackModule outside healer scope",
            )),
            HealingPolicy::RecoverSqlite { .. } => Err(HealerError::NotImplemented(
                "RecoverSqlite owned by SqliteCorruptionHealer",
            )),
            HealingPolicy::EscalateToHuman { .. } => Err(HealerError::NotImplemented(
                "EscalateToHuman outside healer scope",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatcher::Dispatcher;
    use crate::ledger::{HealingLedger, HealingOutcome};
    use serde_json::json;
    use std::sync::Arc;

    fn symptom(kind: &str, detail: serde_json::Value) -> Symptom {
        Symptom {
            source: "aether-telemetry".into(),
            kind: kind.into(),
            detail,
            severity: 2,
        }
    }

    #[tokio::test]
    async fn ignores_unrelated_kinds() {
        let h = TelemetryOverflowHealer::new();
        assert!(h
            .diagnose(&symptom("sync.outbox.stuck", json!({})))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn high_buffer_fill_scores_above_threshold() {
        let h = TelemetryOverflowHealer::new();
        let d = h
            .diagnose(&symptom(
                "telemetry.buffer.overflow",
                json!({ "buffer_depth": 95_000, "buffer_capacity": 100_000 }),
            ))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(d.confidence >= 0.3, "90% buffer fill → 0.3 min");
        assert!(matches!(d.recommended, HealingPolicy::TrimTelemetry { .. }));
    }

    #[tokio::test]
    async fn critical_disk_pressure_alone_triggers_diagnosis() {
        let h = TelemetryOverflowHealer::new();
        let d = h
            .diagnose(&symptom(
                "telemetry.disk.pressure",
                json!({ "disk_free_mb": 50 }),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(d.confidence >= 0.5);
    }

    #[tokio::test]
    async fn zero_capacity_does_not_panic() {
        // Misconfigured telemetry (capacity=0) should report 0.0 for
        // the buffer score rather than divide-by-zero panic.
        let h = TelemetryOverflowHealer::new();
        let d = h
            .diagnose(&symptom(
                "telemetry.buffer.overflow",
                json!({ "buffer_depth": 100, "buffer_capacity": 0 }),
            ))
            .await
            .unwrap();
        // With no disk-free evidence and zero-capacity buffer, total
        // score is zero → no diagnosis.
        assert!(d.is_none());
    }

    #[tokio::test]
    async fn no_evidence_returns_none() {
        let h = TelemetryOverflowHealer::new();
        let d = h
            .diagnose(&symptom("telemetry.buffer.overflow", json!({})))
            .await
            .unwrap();
        assert!(d.is_none());
    }

    #[tokio::test]
    async fn missing_disk_free_does_not_trigger_disk_pressure_score() {
        // Sanity for the `unwrap_or(u64::MAX)` default: a symptom
        // without disk_free_mb must not look like a disk-pressure
        // alarm.
        let h = TelemetryOverflowHealer::new();
        let d = h
            .diagnose(&symptom(
                "telemetry.buffer.overflow",
                json!({ "buffer_depth": 50_000, "buffer_capacity": 100_000 }),
            ))
            .await
            .unwrap();
        // Buffer is at 50% (below noise floor 0.75) → score 0.0;
        // disk default is "infinite free" → score 0.0; total 0.0.
        assert!(d.is_none());
    }

    #[tokio::test]
    async fn dispatcher_gates_destructive_trim_telemetry() {
        // The headline destructive-gating proof: even at maximum
        // confidence, the dispatcher refuses to auto-apply
        // TrimTelemetry on the client. Without this, the healer
        // could quietly drop telemetry windows behind operators'
        // backs — the whole reason the policy is flagged destructive.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        d.add(Arc::new(TelemetryOverflowHealer::new()));

        let event = d
            .handle(symptom(
                "telemetry.buffer.overflow",
                json!({
                    "buffer_depth": 99_000,
                    "buffer_capacity": 100_000,
                    "disk_free_mb": 50,
                }),
            ))
            .await;

        // Skipped, NOT Applied — even at 1.0 confidence.
        assert!(matches!(
            event.outcome,
            HealingOutcome::Skipped { ref reason } if reason.contains("destructive")
        ));
        // The policy is still recorded so operators see what the
        // healer WOULD have done.
        assert!(matches!(event.policy, HealingPolicy::TrimTelemetry { .. }));
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn apply_returns_descriptor_when_invoked_directly() {
        // Even though the dispatcher won't call apply() for
        // destructive policies, the cloud-confirmation path will —
        // so apply must work and emit a useful descriptor.
        let h = TelemetryOverflowHealer::new();
        let desc = h
            .apply(&HealingPolicy::TrimTelemetry {
                keep_recent_hours: 48,
            })
            .await
            .unwrap();
        assert!(desc.contains("48h"));
    }

    #[tokio::test]
    async fn apply_rejects_unrelated_policies() {
        let h = TelemetryOverflowHealer::new();
        assert!(matches!(
            h.apply(&HealingPolicy::RestartService {
                name: "sync".into()
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
    }
}
