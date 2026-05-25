//! Suite builder — the glue from a standard slug to a runnable set of
//! probes and, ultimately, a `ComplianceReport`.
//!
//! The catalog (`catalog::CATALOG`) maps each standard to its control
//! point ids; the probe modules implement individual controls; the runner
//! turns a probe set into a hash-chained report. This module wires them
//! together so a caller (desktop IPC, CLI, Edge Function pre-flight) makes
//! one call instead of hand-assembling probes.
//!
//! ## Coverage
//! Every control point in the catalog now maps to an automated probe —
//! specific probes for the bespoke controls, plus two generic families
//! ([`crate::CURRENCY_CONTROLS`] "artifact on file & current" and
//! [`crate::VIOLATION_CONTROLS`] "zero open violations"). [`build_suite`]
//! still uses `filter_map`, so a control added later without a probe is
//! skipped rather than panicking; the `every_catalog_control_has_a_probe`
//! test guards that we don't regress below full coverage unnoticed.
//!
//! ## Time context
//! Probes split into "point-in-time" (need a `since` window start or an
//! `as_of` reference) and "as of now" (SLA counts). [`ReportWindow`]
//! carries both instants so the whole suite shares one consistent clock,
//! keeping the report reproducible.

use crate::catalog::CATALOG;
use crate::control::Probe;
use crate::evidence::EvidenceSource;
use crate::report::ComplianceReport;
use crate::runner::ProbeRunner;
use crate::{
    ArtifactCurrencyProbe, AuditTrailImmutableProbe, BreachNotificationProbe, ColdChainProbe,
    DsarPipelineProbe, EncryptionAtRestProbe, IncidentLogProbe, KeyRotationProbe,
    PeriodicReviewProbe, ReviewKind, SignatureBindingProbe, ViolationCountProbe, CURRENCY_CONTROLS,
    VIOLATION_CONTROLS,
};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

/// Shared time context for a report run. `since` bounds the evidence
/// window for `*_since` probes; `as_of` is the reference instant for
/// interval/cadence probes. Both are explicit so a report regenerated
/// for the same window is byte-identical.
#[derive(Clone, Copy, Debug)]
pub struct ReportWindow {
    pub since: DateTime<Utc>,
    pub as_of: DateTime<Utc>,
}

impl ReportWindow {
    pub fn new(since: DateTime<Utc>, as_of: DateTime<Utc>) -> Self {
        Self { since, as_of }
    }

    /// Window covering the last `days`, ending now.
    pub fn last_days(days: i64) -> Self {
        let as_of = Utc::now();
        Self {
            since: as_of - Duration::days(days),
            as_of,
        }
    }
}

/// Construct the probe for a control point id, or `None` if no probe is
/// implemented for it yet. Adding a probe = one arm here.
pub fn probe_for(
    control_id: &str,
    evidence: Arc<dyn EvidenceSource>,
    window: ReportWindow,
) -> Option<Arc<dyn Probe>> {
    let probe: Arc<dyn Probe> = match control_id {
        "er-audit-trail-immutable" => {
            Arc::new(AuditTrailImmutableProbe::new(evidence, window.since))
        }
        "is-encryption-at-rest" => Arc::new(EncryptionAtRestProbe::new(evidence, window.since)),
        "is-incident-log" => Arc::new(IncidentLogProbe::new(evidence)),
        "is-key-rotation" => Arc::new(KeyRotationProbe::new(evidence, window.as_of)),
        "fs-cold-chain" => Arc::new(ColdChainProbe::new(evidence, window.since)),
        "is-access-review" => Arc::new(PeriodicReviewProbe::new(
            evidence,
            ReviewKind::AccessReview,
            window.as_of,
        )),
        "qms-internal-audit" => Arc::new(PeriodicReviewProbe::new(
            evidence,
            ReviewKind::InternalAudit,
            window.as_of,
        )),
        "qms-mgmt-review" => Arc::new(PeriodicReviewProbe::new(
            evidence,
            ReviewKind::ManagementReview,
            window.as_of,
        )),
        "dp-breach-notification" => Arc::new(BreachNotificationProbe::new(evidence)),
        "dp-dsar-pipeline" => Arc::new(DsarPipelineProbe::new(evidence)),
        "er-signature-binding" => Arc::new(SignatureBindingProbe::new(evidence)),
        // Generic families: "artifact on file & current" (CURRENCY_CONTROLS)
        // and "zero open violations" (VIOLATION_CONTROLS). One probe per
        // registry entry; the branches are mutually exclusive so `evidence`
        // moves at most once.
        _ => {
            if let Some((cid, max_age)) = CURRENCY_CONTROLS.iter().find(|(c, _)| *c == control_id) {
                return Some(Arc::new(ArtifactCurrencyProbe::new(
                    evidence,
                    cid,
                    *max_age,
                    window.as_of,
                )));
            }
            if let Some(cid) = VIOLATION_CONTROLS.iter().find(|c| **c == control_id) {
                return Some(Arc::new(ViolationCountProbe::new(evidence, cid)));
            }
            return None;
        }
    };
    Some(probe)
}

/// True if a standard with this slug exists in the catalog.
pub fn is_known_standard(standard_slug: &str) -> bool {
    CATALOG.iter().any(|s| s.slug == standard_slug)
}

/// Build the probe set for a standard: every control point that has an
/// implemented probe. Returns an empty vec for an unknown slug or a
/// standard whose controls are all still manual.
pub fn build_suite(
    standard_slug: &str,
    evidence: Arc<dyn EvidenceSource>,
    window: ReportWindow,
) -> Vec<Arc<dyn Probe>> {
    let Some(standard) = CATALOG.iter().find(|s| s.slug == standard_slug) else {
        return Vec::new();
    };
    standard
        .control_point_ids
        .iter()
        .filter_map(|cid| probe_for(cid, evidence.clone(), window))
        .collect()
}

/// Run a full compliance report for a standard against an evidence
/// source. `None` if the slug isn't a known standard; otherwise a report
/// (possibly with zero controls if none are probe-backed yet).
pub async fn run_standard(
    tenant_id: Uuid,
    standard_slug: &str,
    evidence: Arc<dyn EvidenceSource>,
    window: ReportWindow,
) -> Option<ComplianceReport> {
    if !is_known_standard(standard_slug) {
        return None;
    }
    let probes = build_suite(standard_slug, evidence, window);
    Some(ProbeRunner::run(tenant_id, standard_slug, &probes).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;
    use crate::report::OverallStatus;

    fn window() -> ReportWindow {
        ReportWindow::last_days(30)
    }

    fn evidence() -> Arc<dyn EvidenceSource> {
        Arc::new(MockEvidenceSource::new())
    }

    #[test]
    fn iso_27001_suite_covers_its_four_controls() {
        // Every iso-27001 control (access-review, incident-log,
        // encryption, key-rotation) has a probe today.
        let suite = build_suite("iso-27001", evidence(), window());
        assert_eq!(suite.len(), 4);
    }

    #[test]
    fn iso_9001_suite_is_fully_covered() {
        // All 5 iso-9001 controls now have a probe: qms-document-control
        // (currency), qms-mgmt-review + qms-internal-audit (periodic
        // review), qms-non-conformance + qms-capa (violation count).
        let suite = build_suite("iso-9001", evidence(), window());
        assert_eq!(suite.len(), 5);
    }

    #[test]
    fn every_catalog_control_has_a_probe() {
        // "Probe sampai habis": every control point in the catalog maps to
        // an automated probe (specific, currency, or violation). A control
        // added later without a probe will trip this test.
        use crate::control::CONTROL_POINTS;
        for cp in CONTROL_POINTS {
            assert!(
                probe_for(cp.id, evidence(), window()).is_some(),
                "control {} has no probe",
                cp.id
            );
        }
    }

    #[test]
    fn unknown_standard_builds_empty_suite() {
        assert!(build_suite("not-a-standard", evidence(), window()).is_empty());
        assert!(!is_known_standard("not-a-standard"));
    }

    #[test]
    fn probe_for_non_catalog_control_is_none() {
        // An id that isn't a real control point has no probe.
        assert!(probe_for("not-a-real-control", evidence(), window()).is_none());
    }

    #[test]
    fn probe_for_currency_and_violation_controls_are_some() {
        assert!(probe_for("qms-document-control", evidence(), window()).is_some()); // currency
        assert!(probe_for("auto-ppap", evidence(), window()).is_some()); // currency
        assert!(probe_for("qms-non-conformance", evidence(), window()).is_some()); // violation
        assert!(probe_for("dp-encryption-pii", evidence(), window()).is_some());
        // violation
    }

    #[test]
    fn probe_for_mapped_control_is_some() {
        assert!(probe_for("is-key-rotation", evidence(), window()).is_some());
    }

    #[tokio::test]
    async fn run_standard_unknown_is_none() {
        assert!(run_standard(Uuid::nil(), "nope", evidence(), window())
            .await
            .is_none());
    }

    #[tokio::test]
    async fn run_standard_produces_a_verifiable_report() {
        // All-default mock: encryption needs activity it doesn't have, so
        // that control needs review; the report still assembles and its
        // chain verifies.
        let report = run_standard(Uuid::nil(), "iso-27001", evidence(), window())
            .await
            .expect("iso-27001 is a known standard");
        assert_eq!(report.controls.len(), 4);
        report.verify_chain().expect("assembled chain verifies");
        // Status is a real rollup, not hard-coded.
        assert!(matches!(
            report.status,
            OverallStatus::Compliant | OverallStatus::NeedsReview | OverallStatus::NonCompliant
        ));
    }
}
