//! ProbeRunner — execute a set of probes and produce a hash-chained
//! `ComplianceReport`.
//!
//! ## Why a runner crate-side rather than per-caller wiring
//! Every caller (desktop app, CLI tool, Edge Function pre-flight) would
//! otherwise duplicate the same loop: for each probe, run evaluate(),
//! coerce errors into NotApplicable verdicts, hash-link the entry to
//! the previous one, and bundle into a report. Centralizing that here
//! means:
//!
//!  * One place sets the error-coercion policy (any probe Err →
//!    NotApplicable verdict with the error in the evidence string).
//!    Auditors treat "we couldn't verify" as distinct from Pass and
//!    Fail; coercing to NotApplicable surfaces it cleanly.
//!  * One place computes the hash-chain. The chain is the
//!    tamper-evident layer beneath the cloud-side Ed25519 signature
//!    (`compliance-attest` Edge Function): mutating a past verdict
//!    breaks every subsequent hash, which the Edge Function checks
//!    before signing.
//!
//! ## Hash-chain shape
//! Each entry's hash = SHA-256 over:
//!   `prev_hash || control_point_id || verdict_byte || evidence_bytes`
//! The first entry uses a 32-byte zero block as `prev_hash`. The
//! report's final hash is the last entry's hash — that's what the
//! Edge Function signs. Re-verifying a saved report is one fold over
//! the controls array.

use crate::control::{Probe, Verdict};
use crate::report::{ChainedVerdict, ComplianceReport};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

pub struct ProbeRunner;

impl ProbeRunner {
    /// Run every probe and produce the final report. Probes are
    /// executed sequentially — concurrency would make the hash-chain
    /// non-deterministic without per-control IDs in the seed, and
    /// auditors expect serial execution anyway.
    pub async fn run(
        tenant_id: Uuid,
        standard_slug: impl Into<String>,
        probes: &[Arc<dyn Probe>],
    ) -> ComplianceReport {
        let mut chained: Vec<ChainedVerdict> = Vec::with_capacity(probes.len());
        let mut prev_hash: [u8; 32] = [0u8; 32];

        for probe in probes {
            let (verdict, evidence) = match probe.evaluate().await {
                Ok(pair) => pair,
                // Coerce probe errors into NotApplicable verdicts so a
                // single dependency outage doesn't kill the whole
                // report. The evidence string carries the error for
                // triage.
                Err(e) => (Verdict::NotApplicable, format!("probe error: {e}")),
            };

            let control_id = probe.control_point().id.to_string();
            let hash = next_hash(&prev_hash, &control_id, verdict, &evidence);
            chained.push(ChainedVerdict {
                control_point_id: control_id,
                verdict,
                evidence,
                prev_hash: prev_hash.to_vec(),
                hash: hash.to_vec(),
            });
            prev_hash = hash;
        }

        ComplianceReport::from_chained(tenant_id, standard_slug, chained)
    }
}

/// SHA-256 of (prev_hash || control_id_bytes || verdict_byte ||
/// evidence_bytes). Pulled out so the verifier in `report.rs` can use
/// the exact same construction — any drift between writer and reader
/// would silently break attestation verification.
pub(crate) fn next_hash(
    prev: &[u8; 32],
    control_id: &str,
    verdict: Verdict,
    evidence: &str,
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(prev);
    // Length-prefix the variable-length fields so a malicious
    // operator can't merge an evidence string and control ID across
    // a boundary to produce a fake-but-valid hash for a different
    // verdict.
    let id_bytes = control_id.as_bytes();
    h.update((id_bytes.len() as u32).to_le_bytes());
    h.update(id_bytes);
    h.update([verdict_byte(verdict)]);
    let ev_bytes = evidence.as_bytes();
    h.update((ev_bytes.len() as u32).to_le_bytes());
    h.update(ev_bytes);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

/// Stable 1-byte tag per verdict variant. Used inside the hash
/// preimage so a verdict change invalidates the chain even if the
/// evidence string is identical (e.g. "see manual review notes" with
/// a Verdict flipped from Pass to Fail).
fn verdict_byte(v: Verdict) -> u8 {
    match v {
        Verdict::Pass => 0x01,
        Verdict::Fail => 0x02,
        Verdict::NotApplicable => 0x03,
        Verdict::NeedsReview => 0x04,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{ControlPoint, ProbeError, CONTROL_POINTS};
    use crate::report::OverallStatus;
    use async_trait::async_trait;

    /// Probe whose verdict is scripted at construction. Lets us test
    /// the runner against deterministic outcomes without standing up
    /// a real evidence source.
    struct ScriptedProbe {
        control_id: &'static str,
        result: Result<(Verdict, &'static str), &'static str>,
    }

    #[async_trait]
    impl Probe for ScriptedProbe {
        fn control_point(&self) -> &ControlPoint {
            CONTROL_POINTS
                .iter()
                .find(|c| c.id == self.control_id)
                .expect("scripted probe uses a real catalog id")
        }
        async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
            match self.result {
                Ok((v, e)) => Ok((v, e.into())),
                Err(msg) => Err(ProbeError::DependencyMissing(msg.into())),
            }
        }
    }

    fn pass(id: &'static str) -> Arc<dyn Probe> {
        Arc::new(ScriptedProbe {
            control_id: id,
            result: Ok((Verdict::Pass, "ok")),
        })
    }
    fn fail(id: &'static str) -> Arc<dyn Probe> {
        Arc::new(ScriptedProbe {
            control_id: id,
            result: Ok((Verdict::Fail, "broken")),
        })
    }
    fn err(id: &'static str) -> Arc<dyn Probe> {
        Arc::new(ScriptedProbe {
            control_id: id,
            result: Err("dep down"),
        })
    }

    #[tokio::test]
    async fn empty_set_produces_compliant_report() {
        let r = ProbeRunner::run(Uuid::nil(), "iso-9001", &[]).await;
        assert_eq!(r.status, OverallStatus::Compliant);
        assert!(r.controls.is_empty());
    }

    #[tokio::test]
    async fn all_pass_yields_compliant() {
        let r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), pass("qms-mgmt-review")],
        )
        .await;
        assert_eq!(r.status, OverallStatus::Compliant);
        assert_eq!(r.controls.len(), 2);
    }

    #[tokio::test]
    async fn any_fail_yields_non_compliant() {
        let r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), fail("qms-mgmt-review")],
        )
        .await;
        assert_eq!(r.status, OverallStatus::NonCompliant);
    }

    #[tokio::test]
    async fn probe_error_coerces_to_not_applicable() {
        // A single dependency outage shouldn't kill the whole audit —
        // surfaces as NotApplicable so the auditor sees what couldn't
        // be checked rather than getting a silent Pass.
        let r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), err("qms-mgmt-review")],
        )
        .await;
        assert_eq!(r.controls[1].verdict, Verdict::NotApplicable);
        assert!(r.controls[1].evidence.contains("probe error"));
    }

    #[tokio::test]
    async fn hash_chain_links_each_entry_to_predecessor() {
        let r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[
                pass("qms-document-control"),
                pass("qms-mgmt-review"),
                fail("qms-internal-audit"),
            ],
        )
        .await;
        // First entry's prev_hash is the zero seed.
        assert!(r.controls[0].prev_hash.iter().all(|&b| b == 0));
        // Subsequent entries chain.
        assert_eq!(r.controls[1].prev_hash, r.controls[0].hash);
        assert_eq!(r.controls[2].prev_hash, r.controls[1].hash);
    }

    #[tokio::test]
    async fn hash_is_deterministic_for_identical_input() {
        let r1 = ProbeRunner::run(Uuid::nil(), "iso-9001", &[pass("qms-document-control")]).await;
        let r2 = ProbeRunner::run(Uuid::nil(), "iso-9001", &[pass("qms-document-control")]).await;
        // Same probes → same hashes. (UUID + timestamp differ on
        // ComplianceReport but the per-control hashes are pure.)
        assert_eq!(r1.controls[0].hash, r2.controls[0].hash);
    }

    #[tokio::test]
    async fn hash_changes_with_verdict_flip() {
        // The verdict_byte inside the preimage ensures that flipping
        // Pass→Fail (or any other variant) changes the chain hash
        // even if the evidence string is left untouched.
        let r_pass =
            ProbeRunner::run(Uuid::nil(), "iso-9001", &[pass("qms-document-control")]).await;
        // Same control id, same evidence string ("ok" vs "ok"), but
        // verdict differs. Construct the "fail with same evidence"
        // probe manually so the test is precise.
        struct OkEvidenceFail;
        #[async_trait]
        impl Probe for OkEvidenceFail {
            fn control_point(&self) -> &ControlPoint {
                CONTROL_POINTS
                    .iter()
                    .find(|c| c.id == "qms-document-control")
                    .unwrap()
            }
            async fn evaluate(&self) -> Result<(Verdict, String), ProbeError> {
                Ok((Verdict::Fail, "ok".into()))
            }
        }
        let r_fail = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[Arc::new(OkEvidenceFail) as Arc<dyn Probe>],
        )
        .await;
        assert_ne!(
            r_fail.controls[0].hash, r_pass.controls[0].hash,
            "verdict flip must perturb the hash even when evidence is identical"
        );
    }

    #[tokio::test]
    async fn report_verifies_its_own_chain_when_intact() {
        let r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), fail("qms-mgmt-review")],
        )
        .await;
        assert!(r.verify_chain().is_ok());
    }

    #[tokio::test]
    async fn report_detects_a_tampered_verdict() {
        // The tamper-evidence promise: edit a saved verdict, the
        // chain rejects it. This is the property the cloud Edge
        // Function relies on before signing.
        let mut r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), pass("qms-mgmt-review")],
        )
        .await;
        // Mutate the first entry's verdict directly.
        r.controls[0].verdict = Verdict::Fail;
        assert!(r.verify_chain().is_err());
    }

    #[tokio::test]
    async fn report_detects_a_tampered_evidence_string() {
        let mut r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[pass("qms-document-control"), pass("qms-mgmt-review")],
        )
        .await;
        r.controls[0].evidence = "ok with a tiny lie".into();
        assert!(r.verify_chain().is_err());
    }

    #[tokio::test]
    async fn report_detects_a_reordered_entry() {
        // Swapping two entries breaks the chain even if both are
        // individually well-formed, because each entry's prev_hash
        // points to a now-different predecessor.
        let mut r = ProbeRunner::run(
            Uuid::nil(),
            "iso-9001",
            &[
                pass("qms-document-control"),
                fail("qms-mgmt-review"),
                pass("qms-internal-audit"),
            ],
        )
        .await;
        r.controls.swap(0, 1);
        assert!(r.verify_chain().is_err());
    }
}
