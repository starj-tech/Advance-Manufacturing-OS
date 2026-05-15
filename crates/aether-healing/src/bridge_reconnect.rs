//! `BridgeReconnectHealer` — recognize an industrial protocol bridge
//! (OPC-UA or MQTT) stuck in a disconnect/retry loop and recommend a
//! clean restart of the offending bridge.
//!
//! ## Signal sources
//! - `protocols.opcua.disconnect_loop` — the OPC-UA bridge has
//!   failed to maintain a session N times in a row; emitted by
//!   `aether-opcua::OpcUaBridge` when its watchdog reseats N >=
//!   threshold.
//! - `protocols.mqtt.disconnect_loop` — same shape for MQTT,
//!   emitted by `aether-mqtt::MqttBridge`.
//!
//! Evidence carried in `detail`:
//!
//! ```json
//! {
//!   "consecutive_failures": 7,
//!   "since_last_success_ms": 120000
//! }
//! ```
//!
//! ## Why a dedicated healer vs. piggybacking on SyncStuckHealer
//! Two reasons:
//!   * Different subsystem to restart — `opcua` vs `mqtt` vs `sync`.
//!     The healer encodes the kind→service mapping so a future
//!     additional bridge (e.g. EtherCAT) gets its own entry without
//!     touching SyncStuckHealer.
//!   * Different confidence heuristics — a single OPC-UA reconnect
//!     attempt is normal during boot; only sustained failures
//!     warrant a restart. Sync stuckness has a different signature.

use crate::diagnosis::{Diagnosis, Healer, HealerError, Symptom};
use crate::policy::HealingPolicy;
use async_trait::async_trait;

/// Consecutive-failure thresholds. 3 is "noisy network, will probably
/// recover", 5 is "something is wrong", 10 is "definitely broken".
const FAILS_NOISE_FLOOR: u64 = 3;
const FAILS_HIGH: u64 = 5;
const FAILS_CRITICAL: u64 = 10;

/// "Time since last successful connect" thresholds. 30 s is benign,
/// 2 min is concerning, 5 min is a clear outage.
const SINCE_OK_NOISE_FLOOR_MS: u64 = 30_000;
const SINCE_OK_HIGH_MS: u64 = 120_000;
const SINCE_OK_CRITICAL_MS: u64 = 300_000;

pub struct BridgeReconnectHealer;

impl BridgeReconnectHealer {
    pub fn new() -> Self {
        Self
    }

    /// Map the symptom kind to the subsystem name that the
    /// `RestartService` policy should target. Returning `None` means
    /// the kind isn't ours — diagnose() will surface that as no match.
    fn service_for(kind: &str) -> Option<&'static str> {
        match kind {
            "protocols.opcua.disconnect_loop" => Some("opcua"),
            "protocols.mqtt.disconnect_loop" => Some("mqtt"),
            _ => None,
        }
    }

    fn score_fails(fails: u64) -> f32 {
        if fails >= FAILS_CRITICAL {
            0.5
        } else if fails >= FAILS_HIGH {
            0.3
        } else if fails >= FAILS_NOISE_FLOOR {
            0.1
        } else {
            0.0
        }
    }

    fn score_since_ok(since_ms: u64) -> f32 {
        if since_ms >= SINCE_OK_CRITICAL_MS {
            0.5
        } else if since_ms >= SINCE_OK_HIGH_MS {
            0.3
        } else if since_ms >= SINCE_OK_NOISE_FLOOR_MS {
            0.1
        } else {
            0.0
        }
    }
}

impl Default for BridgeReconnectHealer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Healer for BridgeReconnectHealer {
    async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
        let Some(service) = Self::service_for(&symptom.kind) else {
            return Ok(None);
        };

        let fails = symptom
            .detail
            .get("consecutive_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let since_ms = symptom
            .detail
            .get("since_last_success_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let confidence = (Self::score_fails(fails) + Self::score_since_ok(since_ms)).min(1.0);

        // No evidence at all → don't fire. A bridge that reports
        // "disconnect_loop" with zero context is a sign of an
        // upstream telemetry bug, not a real outage.
        if confidence == 0.0 {
            return Ok(None);
        }

        Ok(Some(Diagnosis {
            symptom: symptom.clone(),
            rationale: format!(
                "{service} bridge consecutive_failures={fails} \
                 (high={FAILS_HIGH}, critical={FAILS_CRITICAL}); \
                 since_last_success={since_ms}ms \
                 (high={SINCE_OK_HIGH_MS}, critical={SINCE_OK_CRITICAL_MS})"
            ),
            confidence,
            recommended: HealingPolicy::RestartService {
                name: service.into(),
            },
        }))
    }

    async fn apply(&self, policy: &HealingPolicy) -> Result<String, HealerError> {
        match policy {
            HealingPolicy::RestartService { name } if name == "opcua" || name == "mqtt" => {
                // Production: signal the bridge task's mpsc to drop
                // its session and re-establish. Skeleton: return the
                // descriptor so the dispatcher's ledger row is
                // human-readable.
                Ok(format!("requested clean restart of {name} bridge"))
            }
            HealingPolicy::RestartService { name } => {
                Err(HealerError::NotImplemented(match name.as_str() {
                    "sync" => "RestartService { sync } — owned by SyncStuckHealer",
                    _ => "RestartService — unknown service target",
                }))
            }
            HealingPolicy::RollbackModule { .. } => Err(HealerError::NotImplemented(
                "RollbackModule outside healer scope",
            )),
            HealingPolicy::RecoverSqlite { .. } => Err(HealerError::NotImplemented(
                "RecoverSqlite outside healer scope",
            )),
            HealingPolicy::TrimTelemetry { .. } => Err(HealerError::NotImplemented(
                "TrimTelemetry outside healer scope",
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
    use serde_json::json;

    fn symptom(kind: &str, detail: serde_json::Value) -> Symptom {
        Symptom {
            source: "aether-protocols".into(),
            kind: kind.into(),
            detail,
            severity: 2,
        }
    }

    #[tokio::test]
    async fn ignores_non_protocol_kinds() {
        let h = BridgeReconnectHealer::new();
        assert!(h
            .diagnose(&symptom("sync.outbox.stuck", json!({"depth": 9999})))
            .await
            .unwrap()
            .is_none());
        assert!(h
            .diagnose(&symptom("module.crash", json!({})))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn opcua_loop_recommends_opcua_restart() {
        let h = BridgeReconnectHealer::new();
        let d = h
            .diagnose(&symptom(
                "protocols.opcua.disconnect_loop",
                json!({ "consecutive_failures": 12, "since_last_success_ms": 400_000 }),
            ))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(matches!(
            d.recommended,
            HealingPolicy::RestartService { ref name } if name == "opcua"
        ));
        assert!(d.confidence >= 0.5);
    }

    #[tokio::test]
    async fn mqtt_loop_recommends_mqtt_restart() {
        let h = BridgeReconnectHealer::new();
        let d = h
            .diagnose(&symptom(
                "protocols.mqtt.disconnect_loop",
                json!({ "consecutive_failures": 7, "since_last_success_ms": 150_000 }),
            ))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(matches!(
            d.recommended,
            HealingPolicy::RestartService { ref name } if name == "mqtt"
        ));
    }

    #[tokio::test]
    async fn confidence_combines_evidence_fields() {
        let h = BridgeReconnectHealer::new();
        // critical fails (0.5) + critical since_ok (0.5) = 1.0
        let critical = h
            .diagnose(&symptom(
                "protocols.opcua.disconnect_loop",
                json!({ "consecutive_failures": 10, "since_last_success_ms": 300_000 }),
            ))
            .await
            .unwrap()
            .unwrap();
        // high fails (0.3) + noise floor since_ok (0.1) = 0.4
        let moderate = h
            .diagnose(&symptom(
                "protocols.opcua.disconnect_loop",
                json!({ "consecutive_failures": 5, "since_last_success_ms": 30_000 }),
            ))
            .await
            .unwrap()
            .unwrap();

        assert!(critical.confidence > moderate.confidence);
        assert!((critical.confidence - 1.0).abs() < 1e-3);
        assert!((moderate.confidence - 0.4).abs() < 1e-3);
    }

    #[tokio::test]
    async fn no_evidence_returns_none() {
        // A bridge that reports the kind but ships no evidence is a
        // suspicious upstream — refuse to restart on bare metadata.
        let h = BridgeReconnectHealer::new();
        let d = h
            .diagnose(&symptom("protocols.opcua.disconnect_loop", json!({})))
            .await
            .unwrap();
        assert!(d.is_none());
    }

    #[tokio::test]
    async fn apply_handles_opcua_and_mqtt() {
        let h = BridgeReconnectHealer::new();
        let opcua = h
            .apply(&HealingPolicy::RestartService {
                name: "opcua".into(),
            })
            .await
            .unwrap();
        assert!(opcua.contains("opcua"));
        let mqtt = h
            .apply(&HealingPolicy::RestartService {
                name: "mqtt".into(),
            })
            .await
            .unwrap();
        assert!(mqtt.contains("mqtt"));
    }

    #[tokio::test]
    async fn apply_rejects_sync_restart_with_helpful_error() {
        // Deliberate: this healer doesn't own the sync subsystem.
        // The "owned by SyncStuckHealer" hint helps triage when a
        // dispatcher misroutes (which it shouldn't, but the error
        // message lets future readers debug fast).
        let h = BridgeReconnectHealer::new();
        let err = h
            .apply(&HealingPolicy::RestartService {
                name: "sync".into(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, HealerError::NotImplemented(msg) if msg.contains("SyncStuckHealer")));
    }

    #[tokio::test]
    async fn apply_rejects_unrelated_policies() {
        let h = BridgeReconnectHealer::new();
        assert!(matches!(
            h.apply(&HealingPolicy::TrimTelemetry {
                keep_recent_hours: 24
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
        assert!(matches!(
            h.apply(&HealingPolicy::RecoverSqlite {
                table: "telemetry".into()
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
    }
}
