use crate::control::Verdict;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverallStatus {
    /// All control points pass.
    Compliant,
    /// At least one control point needs review but none failed.
    NeedsReview,
    /// One or more control points failed.
    NonCompliant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlVerdict {
    pub control_point_id: String,
    pub verdict: Verdict,
    pub evidence: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub standard_slug: String,
    pub generated_at: DateTime<Utc>,
    pub status: OverallStatus,
    pub controls: Vec<ControlVerdict>,
    /// Cloud-issued attestation signature (compliance-attest Edge Fn).
    pub attestation_signature: Option<Vec<u8>>,
}

impl ComplianceReport {
    pub fn from_verdicts(
        tenant_id: Uuid,
        standard_slug: impl Into<String>,
        controls: Vec<ControlVerdict>,
    ) -> Self {
        let status = if controls.iter().any(|c| c.verdict == Verdict::Fail) {
            OverallStatus::NonCompliant
        } else if controls.iter().any(|c| c.verdict == Verdict::NeedsReview) {
            OverallStatus::NeedsReview
        } else {
            OverallStatus::Compliant
        };

        Self {
            id: Uuid::now_v7(),
            tenant_id,
            standard_slug: standard_slug.into(),
            generated_at: Utc::now(),
            status,
            controls,
            attestation_signature: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cv(id: &str, v: Verdict) -> ControlVerdict {
        ControlVerdict {
            control_point_id: id.into(),
            verdict: v,
            evidence: "test".into(),
        }
    }

    #[test]
    fn compliant_when_all_pass() {
        let r = ComplianceReport::from_verdicts(
            Uuid::nil(),
            "iso-9001",
            vec![cv("a", Verdict::Pass), cv("b", Verdict::Pass)],
        );
        assert_eq!(r.status, OverallStatus::Compliant);
    }

    #[test]
    fn non_compliant_when_any_fail() {
        let r = ComplianceReport::from_verdicts(
            Uuid::nil(),
            "iso-9001",
            vec![cv("a", Verdict::Pass), cv("b", Verdict::Fail)],
        );
        assert_eq!(r.status, OverallStatus::NonCompliant);
    }

    #[test]
    fn needs_review_when_no_fail_but_review_present() {
        let r = ComplianceReport::from_verdicts(
            Uuid::nil(),
            "iso-9001",
            vec![cv("a", Verdict::Pass), cv("b", Verdict::NeedsReview)],
        );
        assert_eq!(r.status, OverallStatus::NeedsReview);
    }

    #[test]
    fn fail_outranks_review() {
        let r = ComplianceReport::from_verdicts(
            Uuid::nil(),
            "iso-9001",
            vec![cv("a", Verdict::NeedsReview), cv("b", Verdict::Fail)],
        );
        assert_eq!(r.status, OverallStatus::NonCompliant);
    }
}
