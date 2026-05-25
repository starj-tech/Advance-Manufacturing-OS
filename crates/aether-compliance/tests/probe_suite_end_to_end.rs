//! End-to-end: the real probe suite, driven by one shared evidence
//! snapshot, through `ProbeRunner` into a hash-chained `ComplianceReport`.
//!
//! The in-module unit tests exercise each probe and the runner in
//! isolation (the runner with scripted mock probes). This integration
//! test closes the gap they leave: that the *real* probes compose through
//! the public API into a report whose chain verifies — and whose chain
//! catches tampering. It touches only `aether-compliance`'s public
//! surface, which also asserts that surface is sufficient to assemble a
//! report without reaching into crate internals.

use std::sync::Arc;

use aether_compliance::{
    BreachNotificationProbe, ColdChainProbe, EncryptionAtRestProbe, EvidenceSource,
    KeyRotationProbe, MockEvidenceSource, OverallStatus, PeriodicReviewProbe, Probe, ProbeRunner,
    ReviewKind, Verdict,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

/// One snapshot wired so each probe lands on a known, distinct verdict:
/// encryption Pass, key-rotation Fail (overdue), cold-chain Pass,
/// access-review Pass, breach-notification Fail.
fn snapshot(now: chrono::DateTime<Utc>) -> Arc<MockEvidenceSource> {
    let m = Arc::new(MockEvidenceSource::new());
    m.set_encrypted_writes(50); // >= floor -> Pass
    m.set_latest_key_rotation(Some(now - Duration::days(200))); // > 90d -> Fail
    m.set_cold_chain_excursions(0); // none -> Pass
    m.set_latest_review("is-access-review", now - Duration::days(15)); // <= 90d -> Pass
    m.set_breaches_overdue(2); // > 0 -> Fail
    m
}

fn suite(evidence: Arc<MockEvidenceSource>, now: chrono::DateTime<Utc>) -> Vec<Arc<dyn Probe>> {
    let ev = evidence as Arc<dyn EvidenceSource>;
    vec![
        Arc::new(EncryptionAtRestProbe::new(ev.clone(), now)),
        Arc::new(KeyRotationProbe::new(ev.clone(), now)),
        Arc::new(ColdChainProbe::new(ev.clone(), now)),
        Arc::new(PeriodicReviewProbe::new(
            ev.clone(),
            ReviewKind::AccessReview,
            now,
        )),
        Arc::new(BreachNotificationProbe::new(ev)),
    ]
}

#[tokio::test]
async fn real_probe_suite_runs_into_a_verifiable_report() {
    let now = Utc::now();
    let report = ProbeRunner::run(Uuid::nil(), "iso-27001", &suite(snapshot(now), now)).await;

    // Every probe produced exactly one control verdict.
    assert_eq!(report.controls.len(), 5);

    // Two probes fail (key-rotation, breach-notification), so the rolled-up
    // status is non-compliant regardless of the passes.
    assert_eq!(report.status, OverallStatus::NonCompliant);

    // The hash chain the runner produced verifies end to end.
    report
        .verify_chain()
        .expect("freshly built chain must verify");
}

#[tokio::test]
async fn each_control_lands_on_its_expected_verdict() {
    let now = Utc::now();
    let report = ProbeRunner::run(Uuid::nil(), "iso-27001", &suite(snapshot(now), now)).await;

    let verdict_for = |id: &str| {
        report
            .controls
            .iter()
            .find(|c| c.control_point_id == id)
            .map(|c| c.verdict)
    };

    assert_eq!(verdict_for("is-encryption-at-rest"), Some(Verdict::Pass));
    assert_eq!(verdict_for("is-key-rotation"), Some(Verdict::Fail));
    assert_eq!(verdict_for("fs-cold-chain"), Some(Verdict::Pass));
    assert_eq!(verdict_for("is-access-review"), Some(Verdict::Pass));
    assert_eq!(verdict_for("dp-breach-notification"), Some(Verdict::Fail));
}

#[tokio::test]
async fn tampering_a_verdict_breaks_the_chain() {
    let now = Utc::now();
    let mut report = ProbeRunner::run(Uuid::nil(), "iso-27001", &suite(snapshot(now), now)).await;
    assert!(report.verify_chain().is_ok());

    // Flip the first verdict (Pass -> Fail) without recomputing hashes,
    // as a malicious operator rewriting history would. The stored hash
    // no longer matches the recomputed preimage.
    report.controls[0].verdict = Verdict::Fail;
    assert!(report.verify_chain().is_err());
}

#[tokio::test]
async fn an_evidence_outage_degrades_one_control_not_the_whole_report() {
    let now = Utc::now();
    let evidence = snapshot(now);
    // The next evidence call fails; the runner coerces it to
    // NotApplicable rather than aborting the whole report.
    evidence.fail_next("db down");
    let report = ProbeRunner::run(Uuid::nil(), "iso-27001", &suite(evidence, now)).await;

    assert_eq!(report.controls.len(), 5);
    let not_applicable = report
        .controls
        .iter()
        .filter(|c| c.verdict == Verdict::NotApplicable)
        .count();
    assert_eq!(not_applicable, 1, "exactly one control should be degraded");
    // The chain still verifies — degradation is recorded, not hidden.
    report
        .verify_chain()
        .expect("chain intact despite a degraded control");
}
