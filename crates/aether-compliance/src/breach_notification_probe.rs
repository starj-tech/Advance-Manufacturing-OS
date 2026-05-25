//! `BreachNotificationProbe` — verifies every recorded personal-data
//! breach was notified inside the regulatory deadline.
//!
//! Maps to control point `dp-breach-notification` (GDPR Art. 33: notify
//! the supervisory authority within 72 hours of becoming aware). The
//! control is binary: any breach whose 72-hour window elapsed without a
//! logged notification fails it.
//!
//! ## Verdict model
//!   * zero overdue notifications → Pass.
//!   * one or more → Fail. A missed statutory deadline is an objective
//!     breach of the control, not a judgement call.
//!
//! ## Configurable deadline
//! Defaults to 72h (GDPR). Jurisdictions with tighter clocks (some
//! sectoral US/Asia rules) lower it via [`with_deadline_hours`].

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use std::sync::Arc;

const CONTROL_ID: &str = "dp-breach-notification";

/// GDPR Art. 33 statutory window, in hours.
pub const DEFAULT_DEADLINE_HOURS: u32 = 72;

pub struct BreachNotificationProbe {
    evidence: Arc<dyn EvidenceSource>,
    deadline_hours: u32,
}

impl BreachNotificationProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>) -> Self {
        Self {
            evidence,
            deadline_hours: DEFAULT_DEADLINE_HOURS,
        }
    }

    pub fn with_deadline_hours(mut self, deadline_hours: u32) -> Self {
        self.deadline_hours = deadline_hours;
        self
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include dp-breach-notification")
    }
}

#[async_trait]
impl Probe for BreachNotificationProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let overdue = self
            .evidence
            .breaches_unnotified_within(self.deadline_hours)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("breach notifications: {e}")))?;

        if overdue == 0 {
            Ok((
                Verdict::Pass,
                format!(
                    "all breaches notified within the {}h deadline",
                    self.deadline_hours
                ),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "{overdue} breach(es) past the {}h notification deadline",
                    self.deadline_hours
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>) -> BreachNotificationProbe {
        BreachNotificationProbe::new(mock as Arc<dyn EvidenceSource>)
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = BreachNotificationProbe::control_point_static();
    }

    #[tokio::test]
    async fn none_overdue_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_breaches_overdue(0);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
        assert!(msg.contains("72h"));
    }

    #[tokio::test]
    async fn any_overdue_fails() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_breaches_overdue(1);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
    }

    #[tokio::test]
    async fn custom_deadline_is_reported() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_breaches_overdue(0);
        let p =
            BreachNotificationProbe::new(mock as Arc<dyn EvidenceSource>).with_deadline_hours(24);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
        assert!(msg.contains("24h"));
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
