//! `PeriodicReviewProbe` — one probe for the family of "did a required
//! review happen inside its cadence?" controls.
//!
//! Several standards share the same shape: a governance activity must
//! recur on an interval, and the control is satisfied when the most
//! recent occurrence is younger than that interval. Rather than copy the
//! same timestamp-vs-interval logic into a handful of near-identical
//! probes, this one is parameterized by a [`ReviewKind`] that pins the
//! control point and the default cadence.
//!
//! | Kind               | Control point        | Default cadence |
//! |--------------------|----------------------|-----------------|
//! | `AccessReview`     | `is-access-review`   | 90d (quarterly) |
//! | `InternalAudit`    | `qms-internal-audit` | 365d (annual)   |
//! | `ManagementReview` | `qms-mgmt-review`    | 180d (biannual) |
//!
//! ## Verdict model
//!   * within cadence → Pass.
//!   * overdue → Fail (a lapsed interval is a deterministic breach).
//!   * never performed (`None`) → NeedsReview (a brand-new tenant hasn't
//!     had a chance to run the first cycle yet).
//!
//! ## Determinism
//! Age is measured against the caller-supplied `as_of`, never
//! `Utc::now()`, so a regenerated report yields the same verdict.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Which recurring governance review a probe instance checks. Each kind
/// maps to exactly one control point and a sensible default cadence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewKind {
    AccessReview,
    InternalAudit,
    ManagementReview,
}

impl ReviewKind {
    /// Control-point id this review satisfies. Doubles as the evidence
    /// lookup key passed to `EvidenceSource::latest_review`.
    pub fn control_id(self) -> &'static str {
        match self {
            ReviewKind::AccessReview => "is-access-review",
            ReviewKind::InternalAudit => "qms-internal-audit",
            ReviewKind::ManagementReview => "qms-mgmt-review",
        }
    }

    /// Default maximum age (days) before the review is overdue.
    pub fn default_max_age_days(self) -> i64 {
        match self {
            ReviewKind::AccessReview => 90,
            ReviewKind::InternalAudit => 365,
            ReviewKind::ManagementReview => 180,
        }
    }
}

pub struct PeriodicReviewProbe {
    evidence: Arc<dyn EvidenceSource>,
    kind: ReviewKind,
    as_of: DateTime<Utc>,
    max_age_days: i64,
}

impl PeriodicReviewProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, kind: ReviewKind, as_of: DateTime<Utc>) -> Self {
        Self {
            evidence,
            kind,
            as_of,
            max_age_days: kind.default_max_age_days(),
        }
    }

    /// Override the cadence (e.g. a customer contract demanding monthly
    /// access reviews instead of the quarterly default).
    pub fn with_max_age_days(mut self, max_age_days: i64) -> Self {
        self.max_age_days = max_age_days;
        self
    }

    fn control_point_static(kind: ReviewKind) -> &'static ControlPoint {
        let id = kind.control_id();
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == id)
            .expect("control catalog must include every ReviewKind control id")
    }
}

#[async_trait]
impl Probe for PeriodicReviewProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static(self.kind)
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let id = self.kind.control_id();
        let latest = self
            .evidence
            .latest_review(id)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("{id} review: {e}")))?;

        let Some(reviewed_at) = latest else {
            return Ok((
                Verdict::NeedsReview,
                format!("no {id} on record yet; schedule the first review cycle"),
            ));
        };

        let age_days = (self.as_of - reviewed_at).num_days();
        if age_days <= self.max_age_days {
            Ok((
                Verdict::Pass,
                format!(
                    "last {id} {} is {age_days}d old (<= {}d cadence)",
                    reviewed_at.to_rfc3339(),
                    self.max_age_days
                ),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "last {id} {} is {age_days}d old, exceeds {}d cadence; review overdue",
                    reviewed_at.to_rfc3339(),
                    self.max_age_days
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;
    use chrono::Duration;

    #[test]
    fn every_kind_maps_to_a_catalog_control() {
        for kind in [
            ReviewKind::AccessReview,
            ReviewKind::InternalAudit,
            ReviewKind::ManagementReview,
        ] {
            let _ = PeriodicReviewProbe::control_point_static(kind);
        }
    }

    #[tokio::test]
    async fn recent_review_passes() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_review("is-access-review", now - Duration::days(10));
        let p = PeriodicReviewProbe::new(
            mock as Arc<dyn EvidenceSource>,
            ReviewKind::AccessReview,
            now,
        );
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::Pass);
    }

    #[tokio::test]
    async fn overdue_review_fails() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        // 100 days exceeds the 90d access-review default.
        mock.set_latest_review("is-access-review", now - Duration::days(100));
        let p = PeriodicReviewProbe::new(
            mock as Arc<dyn EvidenceSource>,
            ReviewKind::AccessReview,
            now,
        );
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains("overdue"));
    }

    #[tokio::test]
    async fn never_reviewed_needs_review() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        let p = PeriodicReviewProbe::new(
            mock as Arc<dyn EvidenceSource>,
            ReviewKind::InternalAudit,
            now,
        );
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::NeedsReview);
    }

    #[tokio::test]
    async fn cadence_differs_by_kind() {
        // A 200-day-old review passes the annual internal-audit cadence
        // but fails the biannual management-review cadence.
        let now = Utc::now();
        let stamp = now - Duration::days(200);

        let audit_mock = Arc::new(MockEvidenceSource::new());
        audit_mock.set_latest_review("qms-internal-audit", stamp);
        let audit = PeriodicReviewProbe::new(
            audit_mock as Arc<dyn EvidenceSource>,
            ReviewKind::InternalAudit,
            now,
        );
        assert_eq!(audit.evaluate().await.unwrap().0, Verdict::Pass);

        let mgmt_mock = Arc::new(MockEvidenceSource::new());
        mgmt_mock.set_latest_review("qms-mgmt-review", stamp);
        let mgmt = PeriodicReviewProbe::new(
            mgmt_mock as Arc<dyn EvidenceSource>,
            ReviewKind::ManagementReview,
            now,
        );
        assert_eq!(mgmt.evaluate().await.unwrap().0, Verdict::Fail);
    }

    #[tokio::test]
    async fn custom_cadence_overrides_default() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_review("is-access-review", now - Duration::days(40));
        let p = PeriodicReviewProbe::new(
            mock as Arc<dyn EvidenceSource>,
            ReviewKind::AccessReview,
            now,
        )
        .with_max_age_days(30);
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::Fail);
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let p = PeriodicReviewProbe::new(
            mock as Arc<dyn EvidenceSource>,
            ReviewKind::AccessReview,
            now,
        );
        assert!(matches!(
            p.evaluate().await.unwrap_err(),
            ProbeError::DependencyMissing(_)
        ));
    }
}
