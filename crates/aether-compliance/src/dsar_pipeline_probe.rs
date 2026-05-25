//! `DsarPipelineProbe` — verifies data-subject access requests (DSARs)
//! are fulfilled inside the statutory window.
//!
//! Maps to control point `dp-dsar-pipeline` (GDPR Art. 12(3): respond to
//! a DSAR without undue delay and within one month). The control is
//! binary: any DSAR still open past its deadline fails it.
//!
//! ## Verdict model
//!   * zero overdue DSARs → Pass.
//!   * one or more → Fail. A blown statutory deadline is objective, not a
//!     judgement call.
//!
//! Defaults to 30 days; tighter contractual SLAs lower it via
//! [`with_deadline_days`].

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use std::sync::Arc;

const CONTROL_ID: &str = "dp-dsar-pipeline";

/// GDPR Art. 12(3) "within one month", in days.
pub const DEFAULT_DEADLINE_DAYS: u32 = 30;

pub struct DsarPipelineProbe {
    evidence: Arc<dyn EvidenceSource>,
    deadline_days: u32,
}

impl DsarPipelineProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>) -> Self {
        Self {
            evidence,
            deadline_days: DEFAULT_DEADLINE_DAYS,
        }
    }

    pub fn with_deadline_days(mut self, deadline_days: u32) -> Self {
        self.deadline_days = deadline_days;
        self
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include dp-dsar-pipeline")
    }
}

#[async_trait]
impl Probe for DsarPipelineProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let overdue = self
            .evidence
            .dsars_past_deadline(self.deadline_days)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("DSAR pipeline: {e}")))?;

        if overdue == 0 {
            Ok((
                Verdict::Pass,
                format!(
                    "all DSARs fulfilled within the {}-day window",
                    self.deadline_days
                ),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "{overdue} DSAR(s) past the {}-day fulfilment deadline",
                    self.deadline_days
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>) -> DsarPipelineProbe {
        DsarPipelineProbe::new(mock as Arc<dyn EvidenceSource>)
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = DsarPipelineProbe::control_point_static();
    }

    #[tokio::test]
    async fn none_overdue_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_dsars_overdue(0);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn any_overdue_fails() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_dsars_overdue(3);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains('3'));
    }

    #[tokio::test]
    async fn custom_deadline_is_reported() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_dsars_overdue(0);
        let p = DsarPipelineProbe::new(mock as Arc<dyn EvidenceSource>).with_deadline_days(15);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
        assert!(msg.contains("15-day"));
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
