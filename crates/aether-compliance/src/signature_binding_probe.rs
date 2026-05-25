//! `SignatureBindingProbe` — verifies every electronic signature is
//! cryptographically bound to the record it signed.
//!
//! Maps to control point `er-signature-binding` (FDA 21 CFR 11.70:
//! electronic signatures must be linked to their records so they cannot
//! be excised, copied, or transferred to falsify another record). The
//! control is binary: a single unbound (detached) signature fails it.
//!
//! ## Verdict model
//!   * zero unbound signatures → Pass.
//!   * one or more → Fail. An unbound signature is an objective integrity
//!     defect, not a judgement call.

use crate::control::{ControlPoint, Probe, ProbeError, Verdict, CONTROL_POINTS};
use crate::evidence::EvidenceSource;
use async_trait::async_trait;
use std::sync::Arc;

const CONTROL_ID: &str = "er-signature-binding";

pub struct SignatureBindingProbe {
    evidence: Arc<dyn EvidenceSource>,
}

impl SignatureBindingProbe {
    pub fn new(evidence: Arc<dyn EvidenceSource>) -> Self {
        Self { evidence }
    }

    fn control_point_static() -> &'static ControlPoint {
        CONTROL_POINTS
            .iter()
            .find(|cp| cp.id == CONTROL_ID)
            .expect("control catalog must include er-signature-binding")
    }
}

#[async_trait]
impl Probe for SignatureBindingProbe {
    fn control_point(&self) -> &ControlPoint {
        Self::control_point_static()
    }

    async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
        let unbound = self
            .evidence
            .unbound_signatures()
            .await
            .map_err(|e| ProbeError::DependencyMissing(format!("signature binding: {e}")))?;

        if unbound == 0 {
            Ok((
                Verdict::Pass,
                "every electronic signature is bound to its record".to_string(),
            ))
        } else {
            Ok((
                Verdict::Fail,
                format!("{unbound} electronic signature(s) not bound to a record"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::MockEvidenceSource;

    fn probe(mock: Arc<MockEvidenceSource>) -> SignatureBindingProbe {
        SignatureBindingProbe::new(mock as Arc<dyn EvidenceSource>)
    }

    #[test]
    fn catalog_entry_exists() {
        let _ = SignatureBindingProbe::control_point_static();
    }

    #[tokio::test]
    async fn all_bound_passes() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_unbound_signatures(0);
        let (v, _) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Pass);
    }

    #[tokio::test]
    async fn any_unbound_fails() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.set_unbound_signatures(2);
        let (v, msg) = probe(mock).evaluate().await.unwrap();
        assert_eq!(v, Verdict::Fail);
        assert!(msg.contains('2'));
    }

    #[tokio::test]
    async fn evidence_source_failure_propagates() {
        let mock = Arc::new(MockEvidenceSource::new());
        mock.fail_next("db down");
        let err = probe(mock).evaluate().await.unwrap_err();
        assert!(matches!(err, ProbeError::DependencyMissing(_)));
    }
}
