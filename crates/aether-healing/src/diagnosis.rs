use crate::policy::HealingPolicy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Symptom {
    /// Source crate / subsystem that reported the symptom.
    pub source: String,
    /// Short identifier (e.g. `sync.outbox.stuck`, `protocols.opcua.disconnect_loop`).
    pub kind: String,
    /// Free-form structured data (counters, recent error strings, etc).
    pub detail: serde_json::Value,
    /// Severity 0..=3 (info, warn, error, critical).
    pub severity: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnosis {
    pub symptom: Symptom,
    /// Human-readable rationale ("outbox depth >1k, sync_changes lag >10min").
    pub rationale: String,
    /// 0.0..=1.0; the dispatcher picks the highest-scoring diagnosis.
    pub confidence: f32,
    pub recommended: HealingPolicy,
}

#[derive(Debug, Error)]
pub enum HealerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("inconclusive — no healer matched the symptom")]
    Inconclusive,
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

#[async_trait::async_trait]
pub trait Healer: Send + Sync {
    /// Try to diagnose the given symptom. Returning `None` means this
    /// healer doesn't recognize the symptom; another healer may.
    async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError>;

    /// Apply the policy, returning a short outcome description for the
    /// audit ledger. MUST NOT panic; failure should be returned as Err.
    async fn apply(&self, policy: &HealingPolicy) -> Result<String, HealerError>;
}

/// Test/skeleton healer that recognizes nothing and applies nothing.
pub struct NoopHealer;

#[async_trait::async_trait]
impl Healer for NoopHealer {
    async fn diagnose(&self, _symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
        Ok(None)
    }

    async fn apply(&self, _policy: &HealingPolicy) -> Result<String, HealerError> {
        Err(HealerError::NotImplemented(
            "NoopHealer::apply — wired in PR #5",
        ))
    }
}
