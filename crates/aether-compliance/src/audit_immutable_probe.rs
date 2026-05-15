//! `AuditTrailImmutableProbe` — verifies that nothing has UPDATE'd or
//! DELETE'd the audit_log since the prior compliance window.
//!
//! Maps to control point `er-audit-trail-immutable`
//! (electronic-records standards: FDA 21 CFR Part 11, EU GMP Annex 11,
//! ISO 13485 §4.1.6). All three require that audit trails be tamper-
//! evident — any successful mutation is a critical compliance failure
//! that needs immediate triage, not a "review" item.
//!
//! ## Evidence
//! Single boolean from `EvidenceSource::audit_log_tampered_since`. The
//! production implementation watches `audit_log_mutations` (an
//! INSERT-only meta-table that records any DML attempted against
//! `audit_log`) and returns true if any row exists with `op` in
//! `{UPDATE, DELETE}` since the cutoff.
//!
//! ## Verdict mapping
//!  * `tampered == false` → Pass
//!  * `tampered == true` → Fail (NOT NeedsReview — this is a
//!    bright-line failure that an auditor would treat as
//!    disqualifying, so the probe must too)
//!  * dependency error from the evidence source → Err propagated; the
//!    runner records NotApplicable. We refuse to "soft-fail" the
//!    probe to Pass when we couldn't verify, because that's the
//!    failure mode that lets tampered systems slip through audits.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

const CONTROL_ID: &str = "er-audit-trail-immutable";

pub struct AuditTrailImmutableProbe {
    evidence: Arc<dyn EvidenceSource>,
    /// Start of the compliance window. Probes are reproducible: given
    /// the same `since`, two runs produce the same verdict.
    since: DateTime<Utc>,
}

impl AuditTrailImmutableProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, since: DateTime<Utc>) -> Self {
        Self { evidence, since }
    }

    /// Locate this probe's catalog entry. Panics if the catalog has
    /// drifted away from the probe ID — surfaced loudly at test time
    /// (`tests::catalog_entry_exists` below) so a renamed control
    /// breaks the build rather than silently producing a no-op probe.
    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include er-audit-trail-immutable")
    }
}

#[async_trait]
impl Probe for AuditTrailImmutableProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let tampered = self
            .evidence
            .audit_log_tampered_since(self.since)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("audit_log: {e}")))?;
        if tampered {
            Ok((
                Verdict::Fail,
                format!(
                    "UPDATE/DELETE against audit_log detected since {}",
                    self.since.to_rfc3339()
                ),
            ))
        } else {
            Ok((
                Verdict::Pass,
                format!(
                    "audit_log integrity verified — no mutations since {}",
                    self.since.to_rfc3339()
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>, since: DateTime<Utc>) -> AuditTrailImmutableProbe {
        AuditTrailImmutableProbe::new(mock as Arc<dyn EvidenceSource>, since)
    }

    #[test]
    fn catalog_entry_exists() {
        // Build-time link to the catalog — if the ID is renamed without
        // updating this probe, the lookup panics on construction.
        let _ = AuditTrailImmutableProbe::control_point_static();
    }

    #[tokio::test]
    async fn untampered_log_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_tampered(false);
        let (verdict, evidence) = probe(mock, Utc::now()).evaluate().await.unwrap();
        assert_eq!(verdict, Verdict::Pass);
        assert!(evidence.contains("integrity verified"));
    }

    #[tokio::test]
    async fn tampered_log_fails_not_needs_review() {
        // Bright-line: tampering is a Fail, not a soft NeedsReview.
        // An auditor wouldn't accept "we'll check later" for this
        // control, and neither should we.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_tampered(true);
        let (verdict, evidence) = probe(mock, Utc::now()).evaluate().await.unwrap();
        assert_eq!(verdict, Verdict::Fail);
        assert!(evidence.contains("UPDATE/DELETE"));
    }

    #[tokio::test]
    async fn since_is_threaded_through_to_evidence_source() {
        // The probe MUST honor its `since` cutoff so the report is
        // reproducible. Without this we'd silently drift the window.
        let mock = Arc::new(MockEvidenceSource::new());
        let cutoff = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
        probe(mock.clone(), cutoff).evaluate().await.unwrap();
        assert_eq!(
            mock.last_since("audit_log_tampered_since"),
            Some(cutoff),
            "probe must pass its window to the evidence source verbatim"
        );
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates_as_probe_error() {
        // Refusing to soft-fail to Pass when we can't verify — this is
        // the property that prevents a tampered system from sliding
        // through a partial audit.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("transient connection error");
        let err = probe(mock, Utc::now()).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
