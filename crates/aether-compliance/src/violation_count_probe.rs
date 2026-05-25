//! `ViolationCountProbe` — one generic probe for the "zero unresolved
//! violations" family of controls (non-conformances closed inside SLA,
//! CAPAs closed, out-of-spec events corrected, OHS incidents handled,
//! MDR filings on time, PII columns encrypted).
//!
//! Each is satisfied when there are no open violations logged against the
//! control. The probe reads `EvidenceSource::open_violations(control_id)`
//! (backed by `control_violations`, written via
//! `EvidenceWriter::open_violation` / `resolve_violation`).
//!
//! ## Verdict model
//!   * zero open violations → Pass.
//!   * one or more → Fail. An unresolved violation is an objective,
//!     logged defect, not a judgement call.
//!
//! The registry of which controls use this probe lives in
//! [`VIOLATION_CONTROLS`]; the suite builder consults it.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use std::sync::Arc;

/// Controls handled by `ViolationCountProbe` (zero open violations = pass).
pub const VIOLATION_CONTROLS: &[&str] = &[
    "qms-non-conformance",
    "qms-capa",
    "fs-corrective-action",
    "ohs-incident-log",
    "md-mdr-reporting",
    "dp-encryption-pii",
];

/// True if a control is handled by the violation-count probe.
pub fn is_violation_control(control_id: &str) -> bool {
    VIOLATION_CONTROLS.contains(&control_id)
}

pub struct ViolationCountProbe {
    evidence: Arc<dyn EvidenceSource>,
    control_id: &'static str,
}

impl ViolationCountProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, control_id: &'static str) -> Self {
        Self {
            evidence,
            control_id,
        }
    }

    fn control_point_static(control_id: &str) -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == control_id)
            .expect("ViolationCountProbe control_id must be in the catalog")
    }
}

#[async_trait]
impl Probe for ViolationCountProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static(self.control_id)
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let id = self.control_id;
        let open = self
            .evidence
            .open_violations(id)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("{id} violations: {e}")))?;

        if open == 0 {
            Ok((Verdict::Pass, format!("no open {id} violations")))
        } else {
            Ok((
                Verdict::Fail,
                format!("{open} unresolved {id} violation(s)"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    #[test]
    fn every_violation_control_is_in_the_catalog() {
        for cid in VIOLATION_CONTROLS {
            assert!(
                CONTROL_POINTS.iter().any(|cp| cp.id == *cid),
                "violation control {cid} missing from catalog"
            );
        }
    }

    #[tokio::test]
    async fn no_violations_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        let p = ViolationCountProbe::new(mock as Arc<dyn EvidenceSource>, "qms-capa");
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::Pass);
    }

    #[tokio::test]
    async fn open_violations_fail() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_open_violations("qms-non-conformance", 4);
        let p = ViolationCountProbe::new(mock as Arc<dyn EvidenceSource>, "qms-non-conformance");
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains('4'));
    }

    #[tokio::test]
    async fn counts_are_per_control() {
        // A violation against a different control doesn't fail this one.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_open_violations("qms-capa", 2);
        let p = ViolationCountProbe::new(mock as Arc<dyn EvidenceSource>, "fs-corrective-action");
        assert_eq!(p.evaluate().await.unwrap().0, Verdict::Pass);
    }

    #[tokio::test]
    async fn evidence_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let p = ViolationCountProbe::new(mock as Arc<dyn EvidenceSource>, "qms-capa");
        assert!(matches!(
            p.evaluate().await.unwrap_err(),
            ProbeError::DependencyMissing(_)
        ));
    }
}
