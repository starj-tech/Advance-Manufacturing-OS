//! `IncidentLogProbe` — verifies that open security incidents are
//! being triaged inside the response-timeline SLA.
//!
//! Maps to control point `is-incident-log` (ISO 27001 §16, SOC 2
//! CC7.3, NIST SP 800-61). All three require evidence of timely
//! response — having an incident log isn't enough if entries pile up
//! past their SLA. The probe Fails fast if even one incident has
//! aged past the configured SLA, because partial compliance here is
//! widely treated as non-compliance by auditors.
//!
//! ## SLA defaults
//! 24 hours is the conventional default for medium+ severity (mirrors
//! many SOC 2 control narratives). High-severity tenants override
//! this lower in their probe configuration.
//!
//! ## Why per-incident detail
//! When the probe Fails, the evidence string lists the offending
//! incident IDs (not full PII — `OpenIncident::id` only). An auditor
//! reading the report can request triage data for those IDs through
//! the regular access-control flow rather than having sensitive
//! incident details embedded in a long-lived attestation.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use std::sync::Arc;

const CONTROL_ID: &str = "is-incident-log";

/// Default response-timeline SLA in hours. 24 h is the common SOC 2
/// CC7.3 narrative; override per-tenant if a higher-severity profile
/// applies.
pub const DEFAULT_SLA_HOURS: u32 = 24;

pub struct IncidentLogProbe {
    evidence: Arc<dyn EvidenceSource>,
    sla_hours: u32,
}

impl IncidentLogProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>) -> Self {
        Self {
            evidence,
            sla_hours: DEFAULT_SLA_HOURS,
        }
    }

    pub fn with_sla_hours(mut self, sla_hours: u32) -> Self {
        self.sla_hours = sla_hours;
        self
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include is-incident-log")
    }
}

#[async_trait]
impl Probe for IncidentLogProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let breaches = self
            .evidence
            .open_incidents_past_sla(self.sla_hours)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("incidents: {e}")))?;

        if breaches.is_empty() {
            return Ok((
                Verdict::Pass,
                format!("all open incidents within {}h SLA", self.sla_hours),
            ));
        }

        // Cap the evidence string to a reasonable size. A pathological
        // outage that produces hundreds of breaches shouldn't blow up
        // the audit-attestation row to megabytes.
        const MAX_LISTED: usize = 10;
        let listed: Vec<&str> = breaches
            .iter()
            .take(MAX_LISTED)
            .map(|i| i.id.as_str())
            .collect();
        let remainder = breaches.len().saturating_sub(MAX_LISTED);
        let suffix = if remainder > 0 {
            format!(" (+{remainder} more)")
        } else {
            String::new()
        };

        Ok((
            Verdict::Fail,
            format!(
                "{} open incidents past {}h SLA: {}{}",
                breaches.len(),
                self.sla_hours,
                listed.join(", "),
                suffix
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{MockEvidenceSource, OpenIncident};
    use chrono::Utc;

    fn probe(mock: Arc<MockEvidenceSource>) -> IncidentLogProbe {
        IncidentLogProbe::new(mock as Arc<dyn EvidenceSource>)
    }

    fn inc(id: &str) -> OpenIncident {
        OpenIncident {
            id: id.into(),
            opened_at: Utc::now(),
            severity: "high".into(),
        }
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = IncidentLogProbe::control_point_static();
    }

    #[tokio::test]
    async fn empty_open_set_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn single_breach_fails() {
        // No "one strike" tolerance — auditors treat any past-SLA
        // open incident as a control failure.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_open_incidents(vec![inc("INC-1")]);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains("INC-1"));
        assert!(msg.contains("24h SLA"));
    }

    #[tokio::test]
    async fn many_breaches_truncate_evidence_with_remainder_count() {
        // Evidence string must stay compact even when the breach list
        // is pathologically long — protects the attestation row size.
        let mock = Arc::new(MockEvidenceSource::new());
        let incidents: Vec<OpenIncident> = (0..25).map(|i| inc(&format!("INC-{i}"))).collect();
        mock.set_open_incidents(incidents);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        // First 10 listed.
        assert!(msg.contains("INC-0"));
        assert!(msg.contains("INC-9"));
        // Plus "+15 more".
        assert!(msg.contains("+15 more"));
        // The 11th and beyond NOT listed individually.
        assert!(!msg.contains("INC-10"));
    }

    #[tokio::test]
    async fn custom_sla_changes_outcome_via_evidence_source_arg() {
        // The probe passes its SLA to the evidence source verbatim,
        // so tenants on a tighter response-time profile (e.g. 4h)
        // see breaches the default 24h SLA would have missed. We
        // assert the integration with `IncidentLogProbe::with_sla_hours`
        // rather than the mock's filtering (mock returns whatever was
        // set, regardless of SLA — production filters at SQL level).
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_open_incidents(vec![inc("INC-fast")]);
        let p = probe(mock).with_sla_hours(4);
        let (v, msg) = p.evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains("4h SLA"));
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("network");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
