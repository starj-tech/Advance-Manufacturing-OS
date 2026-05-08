use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HealingPolicy {
    /// Restart a Tauri-managed subsystem (sync engine, protocol bridge, etc).
    RestartService { name: String },
    /// Roll back the active module to a previously good version.
    RollbackModule {
        module_id: String,
        target_version: String,
    },
    /// Compact / rebuild a SQLite table when corruption is suspected.
    RecoverSqlite { table: String },
    /// Drop the oldest N rows from the telemetry buffer to relieve
    /// disk pressure. Reported in the audit ledger so operators see
    /// exactly which window was discarded.
    TrimTelemetry { keep_recent_hours: u32 },
    /// Symptom is non-mechanical; require a human (developer / on-call).
    EscalateToHuman { reason: String },
}

impl HealingPolicy {
    /// Returns true if this policy mutates persistent state in a way
    /// that cannot be transparently undone — used by the dispatcher to
    /// require an extra cloud-side confirmation in PR #5.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            HealingPolicy::RecoverSqlite { .. } | HealingPolicy::TrimTelemetry { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_is_safe() {
        assert!(!HealingPolicy::RestartService {
            name: "sync".into()
        }
        .is_destructive());
    }

    #[test]
    fn trim_telemetry_is_destructive() {
        assert!(HealingPolicy::TrimTelemetry {
            keep_recent_hours: 24
        }
        .is_destructive());
    }
}
