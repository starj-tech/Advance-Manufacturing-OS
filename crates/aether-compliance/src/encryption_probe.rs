//! `EncryptionAtRestProbe` — verifies that the encryption-at-rest
//! pipeline is actually being exercised, not just configured. Counts
//! `encrypted=true` writes since the prior compliance window and
//! compares to a minimum-activity threshold.
//!
//! Maps to control point `is-encryption-at-rest` (ISO 27001 §10.1,
//! SOC 2 CC6.1, FDA 21 CFR 11.10(e)). Each of those wants ongoing
//! evidence of encryption operating — a system configured for
//! encryption that never actually encrypts a row is failing the
//! control, even if the encryption module is wired correctly.
//!
//! ## Threshold model
//! Below the floor → NeedsReview ("did the team disable encryption
//! by accident?"). Above the floor → Pass. We intentionally avoid
//! Fail here because a low-activity tenant (small shop, weekend
//! window) might legitimately produce few writes — that's a human
//! judgement call rather than an audit failure.
//!
//! ## Why not Fail on zero?
//! A brand-new tenant with no operations yet would otherwise fail
//! this control before they've done anything. NeedsReview lets the
//! operator click through with a one-sentence justification, while
//! still surfacing the anomaly.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

const CONTROL_ID: &str = "is-encryption-at-rest";

/// Minimum encrypted-write count over the window before the probe is
/// confident the pipeline is live. Configurable per-instance for
/// tenants with atypical write volumes.
pub const DEFAULT_MIN_WRITES: u64 = 10;

pub struct EncryptionAtRestProbe {
    evidence: Arc<dyn EvidenceSource>,
    since: DateTime<Utc>,
    min_writes: u64,
}

impl EncryptionAtRestProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, since: DateTime<Utc>) -> Self {
        Self {
            evidence,
            since,
            min_writes: DEFAULT_MIN_WRITES,
        }
    }

    /// Builder-style override for the activity floor. Tenants with
    /// known low-volume profiles (single-operator dispensary, e.g.)
    /// can drop the floor to 1 so they don't perpetually NeedsReview.
    pub fn with_min_writes(mut self, min_writes: u64) -> Self {
        self.min_writes = min_writes;
        self
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include is-encryption-at-rest")
    }
}

#[async_trait]
impl Probe for EncryptionAtRestProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let writes = self
            .evidence
            .encrypted_column_writes_since(self.since)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("encrypted writes: {e}")))?;
        if writes >= self.min_writes {
            Ok((
                Verdict::Pass,
                format!(
                    "{writes} encrypted-column writes since {} (>= floor {})",
                    self.since.to_rfc3339(),
                    self.min_writes
                ),
            ))
        } else {
            Ok((
                Verdict::NeedsReview,
                format!(
                    "only {writes} encrypted-column writes since {} \
                     (below floor {}); confirm pipeline is enabled",
                    self.since.to_rfc3339(),
                    self.min_writes
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>) -> EncryptionAtRestProbe {
        EncryptionAtRestProbe::new(mock as Arc<dyn EvidenceSource>, Utc::now())
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = EncryptionAtRestProbe::control_point_static();
    }

    #[tokio::test]
    async fn above_floor_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_encrypted_writes(DEFAULT_MIN_WRITES + 1);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn at_floor_passes() {
        // Boundary: exactly the threshold is acceptable. `>=` not `>`.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_encrypted_writes(DEFAULT_MIN_WRITES);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn below_floor_needs_review_not_fail() {
        // Critical design choice: a new tenant with no writes yet
        // should NOT fail audit; they need a human eyeball.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_encrypted_writes(0);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::NeedsReview);
        assert!(msg.contains("below floor"));
    }

    #[tokio::test]
    async fn custom_floor_changes_decision() {
        // Tenant overrode the floor to 1 (low-volume legitimate). One
        // write per window now passes; the default would have flagged.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_encrypted_writes(1);
        let p = EncryptionAtRestProbe::new(mock as Arc<dyn EvidenceSource>, Utc::now())
            .with_min_writes(1);
        let (v, _) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
