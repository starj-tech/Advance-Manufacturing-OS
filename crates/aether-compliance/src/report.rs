use crate::control::Verdict;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
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

/// Legacy non-chained shape. Retained so existing callers that don't
/// need the tamper-evidence property keep working — the runner emits
/// the richer `ChainedVerdict`, but `from_verdicts` still composes a
/// report from plain ones for fixture-driven tests.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlVerdict {
    pub control_point_id: String,
    pub verdict: Verdict,
    pub evidence: String,
}

/// Hash-linked verdict — each carries its predecessor's hash so the
/// audit trail is tamper-evident. The runner emits these; the cloud
/// `compliance-attest` Edge Function re-verifies the chain before
/// signing the final report.
///
/// Both `prev_hash` and `hash` are 32-byte SHA-256 outputs stored as
/// `Vec<u8>` so serde-JSON round-trips through Edge Functions cleanly
/// (no fixed-size-array tagged-enum gymnastics).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainedVerdict {
    pub control_point_id: String,
    pub verdict: Verdict,
    pub evidence: String,
    pub prev_hash: Vec<u8>,
    pub hash: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("entry {index} prev_hash does not match prior entry hash")]
    PrevHashMismatch { index: usize },
    #[error("entry {index} hash does not match recomputed preimage")]
    HashMismatch { index: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub standard_slug: String,
    pub generated_at: DateTime<Utc>,
    pub status: OverallStatus,
    pub controls: Vec<ChainedVerdict>,
    /// Cloud-issued attestation signature (compliance-attest Edge Fn).
    pub attestation_signature: Option<Vec<u8>>,
}

impl ComplianceReport {
    /// Build a report from already-chained verdicts. The runner uses
    /// this — callers who have plain verdicts go through
    /// `from_verdicts` which seeds an empty chain.
    pub fn from_chained(
        tenant_id: Uuid,
        standard_slug: impl Into<String>,
        controls: Vec<ChainedVerdict>,
    ) -> Self {
        let status = overall_status(&controls);
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

    /// Legacy builder for tests / non-runner callers. Promotes plain
    /// `ControlVerdict`s into `ChainedVerdict`s with empty hash
    /// fields. Calling `verify_chain()` on the result will fail —
    /// these reports are explicitly *not* tamper-evident, suitable
    /// only for cases that don't surface to an auditor.
    pub fn from_verdicts(
        tenant_id: Uuid,
        standard_slug: impl Into<String>,
        controls: Vec<ControlVerdict>,
    ) -> Self {
        let promoted = controls
            .into_iter()
            .map(|c| ChainedVerdict {
                control_point_id: c.control_point_id,
                verdict: c.verdict,
                evidence: c.evidence,
                prev_hash: Vec::new(),
                hash: Vec::new(),
            })
            .collect::<Vec<_>>();
        Self::from_chained(tenant_id, standard_slug, promoted)
    }

    /// Re-walk the hash chain and confirm every entry's `hash` is
    /// what the preimage rules in `runner::next_hash` produce. Returns
    /// `Err` on the first inconsistency — that's where the tampering
    /// is, and surfacing the index helps triage.
    pub fn verify_chain(&self) -> Result<(), ChainError> {
        let mut expected_prev: [u8; 32] = [0u8; 32];
        for (idx, entry) in self.controls.iter().enumerate() {
            if entry.prev_hash != expected_prev {
                return Err(ChainError::PrevHashMismatch { index: idx });
            }
            let computed = crate::runner::next_hash(
                &expected_prev,
                &entry.control_point_id,
                entry.verdict,
                &entry.evidence,
            );
            if entry.hash != computed {
                return Err(ChainError::HashMismatch { index: idx });
            }
            expected_prev = computed;
        }
        Ok(())
    }
}

fn overall_status(controls: &[ChainedVerdict]) -> OverallStatus {
    if controls.iter().any(|c| c.verdict == Verdict::Fail) {
        OverallStatus::NonCompliant
    } else if controls.iter().any(|c| c.verdict == Verdict::NeedsReview) {
        OverallStatus::NeedsReview
    } else {
        OverallStatus::Compliant
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

    #[test]
    fn from_verdicts_produces_empty_hashes_that_verify_chain_rejects() {
        // Legacy promotion path is explicitly NOT tamper-evident —
        // verify_chain() must reject it so callers can't accidentally
        // ship a non-attested report to the cloud signer.
        let r =
            ComplianceReport::from_verdicts(Uuid::nil(), "iso-9001", vec![cv("a", Verdict::Pass)]);
        assert!(r.verify_chain().is_err());
    }
}
