//! `ColdChainProbe` — verifies the cold chain held over the reporting
//! window: zero temperature excursions logged since the cutoff.
//!
//! Maps to control point `fs-cold-chain` (ISO 22000 / HACCP, FSSC 22000,
//! BPOM cold-storage rules). The control is binary by nature — a single
//! logged excursion means product spent time outside its safe band, so
//! the verdict is Pass only when the excursion count is exactly zero.
//!
//! ## Why Fail, not NeedsReview
//! Unlike rule-of-thumb probes (encryption write volume, e.g.), a
//! temperature excursion is an objective, logged event: the reading
//! either left the band or it didn't. A non-zero count is a concrete
//! food-safety breach, so Fail is the honest verdict — the operator
//! follows up with the corrective-action control (`fs-corrective-action`),
//! not an audit hand-wave.
//!
//! ## Determinism
//! The excursion count is scoped to `since`, so a report regenerated for
//! the same window returns the same verdict regardless of later readings.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

const CONTROL_ID: &str = "fs-cold-chain";

pub struct ColdChainProbe {
    evidence: Arc<dyn EvidenceSource>,
    since: DateTime<Utc>,
}

impl ColdChainProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>, since: DateTime<Utc>) -> Self {
        Self { evidence, since }
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include fs-cold-chain")
    }
}

#[async_trait]
impl Probe for ColdChainProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let excursions = self
            .evidence
            .cold_chain_excursions_since(self.since)
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("cold-chain excursions: {e}")))?;

        if excursions == 0 {
            Ok((
                Verdict::Pass,
                format!("no cold-chain excursions since {}", self.since.to_rfc3339()),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!(
                    "{excursions} cold-chain excursion(s) since {}; product safety at risk",
                    self.since.to_rfc3339()
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>) -> ColdChainProbe {
        ColdChainProbe::new(mock as Arc<dyn EvidenceSource>, Utc::now())
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = ColdChainProbe::control_point_static();
    }

    #[tokio::test]
    async fn zero_excursions_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_cold_chain_excursions(0);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
        assert!(msg.contains("no cold-chain excursions"));
    }

    #[tokio::test]
    async fn any_excursion_fails() {
        // The control is binary — even one excursion fails it.
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_cold_chain_excursions(1);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
    }

    #[tokio::test]
    async fn many_excursions_reports_the_count() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_cold_chain_excursions(7);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains('7'));
    }

    #[tokio::test]
    async fn probe_scopes_query_to_its_window() {
        // Pin that the probe asks with its own `since`, not some other
        // cutoff — the report's reproducibility depends on it.
        let now = Utc::now();
        let mock = Arc::new(MockEvidenceSource::new());
        let p = ColdChainProbe::new(mock.clone() as Arc<dyn EvidenceSource>, now);
        let _ = p.evaluate().await.unwrap();
        assert_eq!(mock.last_since("cold_chain_excursions_since"), Some(now));
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("sensor feed down");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
