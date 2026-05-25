//! `ArtifactCurrencyProbe` — one generic probe for the large family of
//! "a required document / certificate / record is on file and current"
//! controls (PPAP packs, FAI reports, BPOM registrations, HACCP plans,
//! SCIP submissions, battery passports, …).
//!
//! These controls all share a shape: an artifact must exist and not be
//! older than its validity/refresh interval. Rather than a bespoke probe
//! per control, this one is parameterized by `control_id` + `max_age_days`
//! and reads the generic dated-evidence channel (`EvidenceSource::
//! latest_review`, which `periodic_reviews` backs — a row per "control X
//! satisfied at time T", written via `EvidenceWriter::record_review`).
//!
//! The registry of which controls use this probe (and their intervals)
//! lives in [`CURRENCY_CONTROLS`]; the suite builder consults it.
//!
//! ## Verdict model
//!   * artifact within its interval → Pass.
//!   * older than the interval → Fail (lapsed validity is objective).
//!   * never recorded (`None`) → NeedsReview (a fresh tenant hasn't filed
//!     the first artifact yet — surface it, don't fail them out).
//!
//! Age is measured against the caller-supplied `as_of` for reproducibility.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Controls handled by `ArtifactCurrencyProbe`, paired with the maximum
/// age (days) before the artifact is considered stale. Intervals follow
/// the typical validity of each artifact (annual reviews, multi-year
/// certificates). The suite builder instantiates one probe per entry.
pub const CURRENCY_CONTROLS: &[(&str, i64)] = &[
    // QMS / general documentation
    ("qms-document-control", 365),
    // Environmental
    ("env-aspects-register", 365),
    ("env-emissions-tracking", 30),
    ("em-baseline", 1095),
    ("em-significant-uses", 365),
    // Food safety
    ("fs-haccp-plan", 365),
    ("fs-prp-management", 365),
    ("fs-food-fraud", 365),
    ("fs-recall-test", 180),
    ("fs-ccp-monitoring", 1),
    // OHS
    ("ohs-jha", 365),
    ("ohs-ppe-issuance", 365),
    // Medical devices
    ("md-design-controls", 365),
    ("md-dhf-link", 365),
    ("md-clinical-evaluation", 365),
    ("md-udi-marking", 365),
    // Automotive
    ("auto-ppap", 1095),
    ("auto-pfmea", 365),
    ("auto-msa", 365),
    ("auto-spc", 7),
    // Aerospace
    ("as-fai", 1825),
    ("as-config-mgmt", 365),
    ("as-counterfeit-parts", 365),
    // Electronic records / data protection
    ("er-system-validation", 365),
    ("dp-lawful-basis", 365),
    // Substances
    ("sub-decl-rohs", 365),
    ("sub-decl-reach", 365),
    ("sub-scip-submission", 365),
    // BPOM / SNI (Indonesia)
    ("bpom-registration", 1825),
    ("bpom-gmp", 1095),
    ("bpom-halal-traceability", 1095),
    ("sni-cert-record", 1460),
    ("sni-product-mark", 365),
    // FSC chain of custody
    ("fsc-coc-claim", 365),
    ("fsc-volume-summary", 365),
    // Cosmetics
    ("cos-batch-record", 365),
    ("cos-personnel-training", 365),
    // Battery passport
    ("bat-passport", 1825),
    ("bat-recycled-content", 365),
    ("bat-cfp", 365),
];

/// Max age (days) for a control in [`CURRENCY_CONTROLS`], if present.
pub fn currency_interval(control_id: &str) -> Option<i64> {
    CURRENCY_CONTROLS
        .iter()
        .find(|(c, _)| *c == control_id)
        .map(|(_, days)| *days)
}

pub struct ArtifactCurrencyProbe {
    evidence: Arc<dyn EvidenceSource>,
    control_id: &'static str,
    max_age_days: i64,
    as_of: DateTime<Utc>,
}

impl ArtifactCurrencyProbe {
    pub fn new(
        evidence: Arc<dyn EvidenceSource>,
        control_id: &'static str,
        max_age_days: i64,
        as_of: DateTime<Utc>,
    ) -> Self {
        Self {
            evidence,
            control_id,
            max_age_days,
            as_of,
        }
    }

    fn control_point_static(control_id: &str) -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == control_id)
            .expect("ArtifactCurrencyProbe control_id must be in the catalog")
    }
}

#[async_trait]
impl Probe for ArtifactCurrencyProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static(self.control_id)
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let id = self.control_id;
        let latest = self
            .evidence
            .latest_review(id)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("{id} artifact: {e}")))?;

        let Some(produced_at) = latest else {
            return Ok((
                Verdict::NeedsReview,
                format!("no {id} artifact on record yet; file the first one"),
            ));
        };

        let age_days = (self.as_of - produced_at).num_days();
        if age_days <= self.max_age_days {
            Ok((
                Verdict::Pass,
                format!(
                    "{id} artifact {} is {age_days}d old (<= {}d validity)",
                    produced_at.to_rfc3339(),
                    self.max_age_days
                ),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "{id} artifact {} is {age_days}d old, exceeds {}d validity; refresh it",
                    produced_at.to_rfc3339(),
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
    fn every_currency_control_is_in_the_catalog() {
        for (cid, _) in CURRENCY_CONTROLS {
            assert!(
                CONTROL_POINTS.iter().any(|cp| cp.id == *cid),
                "currency control {cid} missing from catalog"
            );
        }
    }

    #[test]
    fn currency_controls_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (cid, _) in CURRENCY_CONTROLS {
            assert!(seen.insert(*cid), "duplicate currency control {cid}");
        }
    }

    #[tokio::test]
    async fn fresh_artifact_passes() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_review("auto-ppap", now - Duration::days(100));
        let p = ArtifactCurrencyProbe::new(mock as Arc<dyn EvidenceSource>, "auto-ppap", 1095, now);
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::Pass);
    }

    #[tokio::test]
    async fn stale_artifact_fails() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_latest_review("bpom-gmp", now - Duration::days(1200));
        let p = ArtifactCurrencyProbe::new(mock as Arc<dyn EvidenceSource>, "bpom-gmp", 1095, now);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains("exceeds"));
    }

    #[tokio::test]
    async fn missing_artifact_needs_review() {
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        let p =
            ArtifactCurrencyProbe::new(mock as Arc<dyn EvidenceSource>, "fs-haccp-plan", 365, now);
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::NeedsReview);
    }

    #[test]
    fn currency_interval_lookup() {
        assert_eq!(currency_interval("as-fai"), Some(1825));
        assert_eq!(currency_interval("is-key-rotation"), None);
    }
}
