use crate::diagnosis::Symptom;
use crate::policy::HealingPolicy;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealingEvent {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    pub symptom: Symptom,
    pub policy: HealingPolicy,
    pub outcome: HealingOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum HealingOutcome {
    Applied { detail: String },
    Skipped { reason: String },
    Failed { error: String },
}

/// Append-only in-memory ledger. Real impl writes to `audit_log` table
/// via aether-db so the cloud audit trail is unbroken.
#[derive(Default)]
pub struct HealingLedger {
    events: Mutex<Vec<HealingEvent>>,
}

impl HealingLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, event: HealingEvent) {
        self.events
            .lock()
            .expect("HealingLedger mutex poisoned")
            .push(event);
    }

    pub fn snapshot(&self) -> Vec<HealingEvent> {
        self.events
            .lock()
            .expect("HealingLedger mutex poisoned")
            .clone()
    }

    pub fn len(&self) -> usize {
        self.events
            .lock()
            .expect("HealingLedger mutex poisoned")
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_is_append_only() {
        let l = HealingLedger::new();
        assert!(l.is_empty());
        let event = HealingEvent {
            id: Uuid::nil(),
            at: Utc::now(),
            symptom: Symptom {
                source: "test".into(),
                kind: "x".into(),
                detail: serde_json::json!({}),
                severity: 0,
            },
            policy: HealingPolicy::RestartService {
                name: "sync".into(),
            },
            outcome: HealingOutcome::Applied {
                detail: "ok".into(),
            },
        };
        l.record(event);
        assert_eq!(l.len(), 1);
    }
}
