//! `SqliteCorruptionHealer` — recognize SQLite corruption / WAL
//! integrity failures and recommend a destructive table recovery.
//!
//! ## Signal sources
//! - `db.sqlite.corruption` — `PRAGMA integrity_check` returned
//!   non-`ok` rows, or a query failed with
//!   `SQLITE_CORRUPT`/`SQLITE_NOTADB`.
//! - `db.sqlite.wal_check_failed` — the WAL file failed checkpoint
//!   or `wal_checkpoint(FULL)` couldn't replay cleanly.
//!
//! Evidence carried in `detail`:
//!
//! ```json
//! {
//!   "table": "telemetry_samples",
//!   "error": "database disk image is malformed",
//!   "consecutive_failures": 3
//! }
//! ```
//!
//! ## Destructive policy gating
//! Always recommends `HealingPolicy::RecoverSqlite { table }`, which
//! `is_destructive() == true`. Recovery typically means
//! `DROP TABLE … ; CREATE TABLE … ;` plus a re-sync from server — the
//! local row history for that table is lost. The dispatcher refuses
//! to auto-apply; the cloud-confirmation path (PR #5) is the only
//! authorized invocation route.
//!
//! ## Why honor `table` from the symptom
//! Corruption is usually localized — `telemetry_samples` going bad
//! shouldn't trigger a full DB nuke. The healer threads the affected
//! table name through to the policy so the recovery action is
//! surgical.

use crate::diagnosis::{Diagnosis, Healer, HealerError, Symptom};
use crate::policy::HealingPolicy;
use async_trait::async_trait;

/// Even one corrupt query is alarming; three is a clear signal the
/// table can't be read at all. We don't have a "noise floor" lower
/// than 1 here — there's no benign cause for `SQLITE_CORRUPT`.
const FAILS_LOW: u64 = 1;
const FAILS_HIGH: u64 = 2;
const FAILS_CRITICAL: u64 = 3;

/// Table fallback when the symptom didn't carry one. Picking a
/// sentinel that isn't a real table makes the failure visible (any
/// downstream consumer that doesn't sanity-check the name will
/// fail loudly).
const UNKNOWN_TABLE: &str = "_unknown_";

pub struct SqliteCorruptionHealer;

impl SqliteCorruptionHealer {
    pub fn new() -> Self {
        Self
    }

    fn score_fails(fails: u64) -> f32 {
        // 0.4 minimum if there's even one corruption signal — this
        // condition doesn't have a benign baseline like sync lag
        // does. Three or more failures saturates at 1.0.
        if fails >= FAILS_CRITICAL {
            1.0
        } else if fails >= FAILS_HIGH {
            0.7
        } else if fails >= FAILS_LOW {
            0.4
        } else {
            0.0
        }
    }
}

impl Default for SqliteCorruptionHealer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Healer for SqliteCorruptionHealer {
    async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
        if !matches!(
            symptom.kind.as_str(),
            "db.sqlite.corruption" | "db.sqlite.wal_check_failed"
        ) {
            return Ok(None);
        }
        // Default to 1 consecutive failure if the field is absent —
        // the kind alone implies at least one signal happened. This
        // is the opposite stance from the more-cautious healers
        // (sync/telemetry/bridge) because corruption can't be a
        // false alarm: the underlying check either returned a
        // corruption error or it didn't.
        let fails = symptom
            .detail
            .get("consecutive_failures")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        let table = symptom
            .detail
            .get("table")
            .and_then(|v| v.as_str())
            .unwrap_or(UNKNOWN_TABLE)
            .to_string();

        let error = symptom
            .detail
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("<unspecified>");

        let confidence = Self::score_fails(fails);

        Ok(Some(Diagnosis {
            symptom: symptom.clone(),
            rationale: format!(
                "sqlite corruption on table='{table}': '{error}'; \
                 consecutive_failures={fails}"
            ),
            confidence,
            recommended: HealingPolicy::RecoverSqlite { table },
        }))
    }

    async fn apply(&self, policy: &HealingPolicy) -> Result<String, HealerError> {
        match policy {
            HealingPolicy::RecoverSqlite { table } => {
                // Production path: invoked only by the cloud-confirmed
                // pipeline. Will run `DROP TABLE` + `CREATE TABLE …`
                // via aether-db, then trigger a fresh sync_pull for
                // the affected entity. Skeleton emits the descriptor.
                Ok(format!(
                    "would recover sqlite table '{table}' \
                     (drop+recreate then full re-sync; cloud-gated)"
                ))
            }
            HealingPolicy::RestartService { .. } => Err(HealerError::NotImplemented(
                "RestartService outside healer scope",
            )),
            HealingPolicy::RollbackModule { .. } => Err(HealerError::NotImplemented(
                "RollbackModule outside healer scope",
            )),
            HealingPolicy::TrimTelemetry { .. } => Err(HealerError::NotImplemented(
                "TrimTelemetry owned by TelemetryOverflowHealer",
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
            source: "aether-db".into(),
            kind: kind.into(),
            detail,
            severity: 3,
        }
    }

    #[tokio::test]
    async fn ignores_unrelated_kinds() {
        let h = SqliteCorruptionHealer::new();
        assert!(h
            .diagnose(&symptom("sync.outbox.stuck", json!({})))
            .await
            .unwrap()
            .is_none());
        assert!(h
            .diagnose(&symptom("opcua.disconnect_loop", json!({})))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn recognizes_corruption_with_table_in_policy() {
        let h = SqliteCorruptionHealer::new();
        let d = h
            .diagnose(&symptom(
                "db.sqlite.corruption",
                json!({
                    "table": "telemetry_samples",
                    "error": "database disk image is malformed",
                    "consecutive_failures": 2,
                }),
            ))
            .await
            .unwrap()
            .expect("should recognize");
        assert!(matches!(
            d.recommended,
            HealingPolicy::RecoverSqlite { ref table } if table == "telemetry_samples"
        ));
        assert!(d.confidence >= 0.5);
    }

    #[tokio::test]
    async fn wal_check_failure_also_recognized() {
        let h = SqliteCorruptionHealer::new();
        let d = h
            .diagnose(&symptom(
                "db.sqlite.wal_check_failed",
                json!({ "table": "outbox" }),
            ))
            .await
            .unwrap();
        assert!(d.is_some());
    }

    #[tokio::test]
    async fn single_failure_still_yields_positive_confidence() {
        // Unlike the other healers, corruption can't have a benign
        // baseline. One signal is enough for a 0.4 diagnosis — above
        // the dispatcher floor of 0.1 — so the cloud-confirmation
        // path always sees these.
        let h = SqliteCorruptionHealer::new();
        let d = h
            .diagnose(&symptom(
                "db.sqlite.corruption",
                json!({ "table": "x", "consecutive_failures": 1 }),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(d.confidence >= 0.4);
    }

    #[tokio::test]
    async fn missing_failure_count_defaults_to_one() {
        // The kind itself implies a signal — silently dropping the
        // diagnosis when the field is absent would hide real
        // corruption from the audit trail.
        let h = SqliteCorruptionHealer::new();
        let d = h
            .diagnose(&symptom(
                "db.sqlite.corruption",
                json!({ "table": "x", "error": "malformed" }),
            ))
            .await
            .unwrap();
        assert!(d.is_some());
    }

    #[tokio::test]
    async fn missing_table_falls_back_to_unknown_sentinel() {
        let h = SqliteCorruptionHealer::new();
        let d = h
            .diagnose(&symptom(
                "db.sqlite.corruption",
                json!({ "consecutive_failures": 3 }),
            ))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            d.recommended,
            HealingPolicy::RecoverSqlite { ref table } if table == UNKNOWN_TABLE
        ));
    }

    #[tokio::test]
    async fn dispatcher_gates_destructive_recover_sqlite() {
        // RecoverSqlite is destructive — dispatcher MUST refuse to
        // auto-apply. Pair with the TelemetryOverflowHealer's
        // destructive-gating test to cover both flavors of
        // is_destructive() policy.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        d.add(Arc::new(SqliteCorruptionHealer::new()));

        let event = d
            .handle(symptom(
                "db.sqlite.corruption",
                json!({
                    "table": "audit_log",
                    "error": "database disk image is malformed",
                    "consecutive_failures": 5,
                }),
            ))
            .await;

        assert!(matches!(
            event.outcome,
            HealingOutcome::Skipped { ref reason } if reason.contains("destructive")
        ));
        assert!(matches!(
            event.policy,
            HealingPolicy::RecoverSqlite { ref table } if table == "audit_log"
        ));
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn apply_emits_table_name_in_descriptor() {
        let h = SqliteCorruptionHealer::new();
        let desc = h
            .apply(&HealingPolicy::RecoverSqlite {
                table: "boms".into(),
            })
            .await
            .unwrap();
        assert!(desc.contains("boms"));
    }

    #[tokio::test]
    async fn apply_rejects_unrelated_policies() {
        let h = SqliteCorruptionHealer::new();
        assert!(matches!(
            h.apply(&HealingPolicy::TrimTelemetry {
                keep_recent_hours: 24
            })
            .await,
            Err(HealerError::NotImplemented(msg)) if msg.contains("TelemetryOverflowHealer")
        ));
        assert!(matches!(
            h.apply(&HealingPolicy::RestartService {
                name: "sync".into()
            })
            .await,
            Err(HealerError::NotImplemented(_))
        ));
    }
}
