//! `KeyRotationProbe` — verifies that data-encryption keys (DEKs) are
//! being rotated inside the policy interval, not just once at setup.
//!
//! Maps to control point `is-key-rotation` (ISO 27001 §10.1, SOC 2
//! CC6.1, PCI-DSS 3.6.4). Each wants evidence that key material is
//! refreshed on a cadence — a tenant that encrypted everything with a
//! single key two years ago is failing the control even though
//! encryption-at-rest itself looks healthy.
//!
//! ## Threshold model
//! The newest rotation timestamp is compared against the probe's
//! `as_of` reference:
//!
//!   * within `max_age_days` → Pass.
//!   * older than `max_age_days` → Fail. Unlike the encryption-activity
//!     probe (which uses NeedsReview for low volume), an overdue
//!     rotation is a deterministic policy breach: the interval either
//!     elapsed or it didn't, so Fail is the honest verdict.
//!   * never rotated (`None`) → NeedsReview. A brand-new tenant hasn't
//!     had a chance to rotate yet; surface it for a human rather than
//!     failing them out of the gate.
//!
//! ## Determinism
//! The comparison uses the caller-supplied `as_of`, never `Utc::now()`,
//! so a report regenerated for the same window yields the same verdict
//! (see the `control.rs` trait contract).

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

const CONTROL_ID: &str = "is-key-rotation";

/// Maximum age of the newest DEK rotation before the control fails.
/// Defaults to one quarter (90 days), matching the control catalog's
/// "rotated quarterly" wording.
pub const DEFAULT_MAX_AGE_DAYS: i64 = 90;

pub struct KeyRotationProbe {
    evidence: Arc<dyn EvidenceSource>,
    as_of: DateTime<Utc>,
    max_age_days: i64,
}

impl KeyRotationProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, as_of: DateTime<Utc>) -> Self {
        Self {
            evidence,
            as_of,
            max_age_days: DEFAULT_MAX_AGE_DAYS,
        }
    }

    /// Builder-style override for the rotation interval. Standards with
    /// stricter cadences (monthly) or tenants under a tighter contract
    /// can lower the ceiling.
    pub fn with_max_age_days(mut self, max_age_days: i64) -> Self {
        self.max_age_days = max_age_days;
        self
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include is-key-rotation")
    }
}

#[async_trait]
impl Probe for KeyRotationProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let latest = self
            .evidence
            .latest_key_rotation()
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("key rotation: {e}")))?;

        let Some(rotated_at) = latest else {
            return Ok((
                Verdict::NeedsReview,
                "no key rotation recorded yet; confirm DEK epoch tracking is enabled".to_string(),
            ));
        };

        // `num_days` truncates toward zero; a rotation in the future
        // relative to `as_of` yields a negative age and passes.
        let age_days = (self.as_of - rotated_at).num_days();
        if age_days <= self.max_age_days {
            Ok((
                Verdict::Pass,
                format!(
                    "last DEK rotation {} is {age_days}d old (<= {}d interval)",
                    rotated_at.to_rfc3339(),
                    self.max_age_days
                ),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "last DEK rotation {} is {age_days}d old, exceeds {}d interval; rotate now",
                    rotated_at.to_rfc3339(),
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

    fn at(days_ago: i64, now: DateTime<Utc>) -> DateTime<Utc> {
        now - Duration::days(days_ago)
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = KeyRotationProbe::control_point_static();
    }

    #[tokio::test]
    async fn recent_rotation_passes() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(Some(at(30, now)));
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let (v, _) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn at_interval_boundary_passes() {
        // Exactly max_age_days old is still acceptable (`<=`).
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(Some(at(DEFAULT_MAX_AGE_DAYS, now)));
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let (v, _) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn overdue_rotation_fails() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(Some(at(DEFAULT_MAX_AGE_DAYS + 1, now)));
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains("exceeds"));
    }

    #[tokio::test]
    async fn never_rotated_needs_review_not_fail() {
        // A fresh tenant with no rotation history gets a human eyeball,
        // not an audit failure.
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(None);
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::NeedsReview);
        assert!(msg.contains("no key rotation"));
    }

    #[tokio::test]
    async fn custom_interval_tightens_the_decision() {
        // A 31-day-old rotation passes the default 90d window but fails
        // a tenant on a stricter monthly (30d) cadence.
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(Some(at(31, now)));
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now).with_max_age_days(30);
        let (v, _) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
    }

    #[tokio::test]
    async fn verdict_is_reproducible_across_runs() {
        // Same snapshot + same as_of → identical verdict, even though
        // wall-clock time advances between the two calls.
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_key_rotation(Some(at(45, now)));
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let first = p.evaluate().await.unwrap();
        let second = p.evaluate().await.unwrap();
        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let p = KeyRotationProbe::new(mock as Arc<dyn EvidenceSource>, now);
        let err = p.evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
