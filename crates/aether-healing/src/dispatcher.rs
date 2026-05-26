//! Dispatcher — route a `Symptom` through a fleet of `Healer`s, pick
//! the highest-confidence diagnosis, apply (or skip), and record to
//! the audit ledger.
//!
//! ## Why a dispatcher rather than calling `Healer::diagnose` directly?
//! Symptoms surface from many subsystems (sync, protocols, telemetry,
//! db) and the right diagnostic move is usually "ask every healer that
//! has an opinion and let the most confident one win". Centralizing
//! that pattern here means:
//!
//!   * One place enforces the "destructive policies need cloud
//!     confirmation, never auto-apply on the client" rule.
//!   * One place writes the audit event (applied / skipped / failed)
//!     so the ledger semantics are uniform.
//!   * The "no healer matched" outcome is materialized as a
//!     `Skipped { reason: "inconclusive" }` ledger row rather than a
//!     bare error — operators see *that* the symptom arrived even
//!     when no automated remediation fired.
//!
//! ## Selection rule
//! Highest confidence wins; ties resolved by insertion order (the
//! healer added first to the dispatcher). Confidence below 0.1 is
//! treated as "no opinion" — a healer that returns a 0.05 diagnosis
//! is asking to be ignored.

use crate::diagnosis::{Diagnosis, Healer, HealerError, Symptom};
use crate::ledger::{HealingEvent, HealingLedger, HealingOutcome};
use crate::policy::HealingPolicy;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Confidence floor — diagnoses below this are treated as "the healer
/// recognized the symptom but isn't sure", which we treat the same as
/// "no diagnosis at all" rather than risk applying a low-confidence
/// remediation.
const MIN_CONFIDENCE: f32 = 0.1;

/// Cloud-/operator-issued approval to apply a *destructive* remediation
/// that the dispatcher would otherwise only ever skip. `approved` must
/// equal the policy the dispatcher diagnoses on this run — a confirmation
/// for a different (e.g. stale) policy is rejected, so an approval can't
/// be replayed to apply an unexpected remediation.
#[derive(Clone, Debug)]
pub struct Confirmation {
    pub approved: HealingPolicy,
    pub confirmed_by: String,
}

/// Holds a fleet of healers + the shared audit ledger. Cheap to
/// construct; expensive Healers (e.g. ones holding DB pools) are
/// shared via `Arc`.
pub struct Dispatcher {
    healers: Vec<Arc<dyn Healer>>,
    ledger: Arc<HealingLedger>,
}

impl Dispatcher {
    pub fn new(ledger: Arc<HealingLedger>) -> Self {
        Self {
            healers: Vec::new(),
            ledger,
        }
    }

    /// Register a healer. Order is preserved for tie-breaking when two
    /// healers produce the same confidence — the one added first wins.
    pub fn add(&mut self, healer: Arc<dyn Healer>) {
        self.healers.push(healer);
    }

    /// Number of registered healers. Useful for "did boot wire the
    /// whole fleet correctly?" smoke checks.
    pub fn healer_count(&self) -> usize {
        self.healers.len()
    }

    /// Run every healer's `diagnose` against the symptom and return
    /// the best non-trivial diagnosis along with the healer that
    /// produced it. Ignores diagnoses below `MIN_CONFIDENCE`. Returns
    /// `Ok(None)` when no healer cared.
    ///
    /// Errors from individual healers are bubbled — one broken healer
    /// shouldn't poison the whole pipeline silently. The caller (e.g.
    /// `handle`) decides how to surface them.
    async fn select(&self, symptom: &Symptom) -> Result<Option<(usize, Diagnosis)>, HealerError> {
        let mut best: Option<(usize, Diagnosis)> = None;
        for (idx, h) in self.healers.iter().enumerate() {
            let Some(d) = h.diagnose(symptom).await? else {
                continue;
            };
            if d.confidence < MIN_CONFIDENCE {
                continue;
            }
            // Strict `>` so insertion order breaks ties — the healer
            // added first stays the winner on equal confidence.
            match &best {
                Some((_, current)) if d.confidence <= current.confidence => {}
                _ => best = Some((idx, d)),
            }
        }
        Ok(best)
    }

    /// Diagnose AND (where safe) apply. Always records an event to the
    /// ledger — even the "no healer matched" path — and returns the
    /// recorded event so callers can render it in the UI without
    /// re-querying the ledger.
    ///
    /// Destructive policies (`HealingPolicy::is_destructive()`) are
    /// NEVER auto-applied on the client. They surface as a `Skipped`
    /// outcome so an operator (or, in PR #5, an Edge Function with
    /// elevated trust) can confirm.
    pub async fn handle(&self, symptom: Symptom) -> HealingEvent {
        self.run(symptom, None).await
    }

    /// Like [`Self::handle`], but carries a [`Confirmation`]. A
    /// destructive remediation is applied only when the confirmation
    /// approves the exact policy the dispatcher diagnoses; a missing or
    /// mismatched confirmation still skips. Safe policies apply as usual.
    /// This is the second half of the destructive-policy flow: the client
    /// surfaces the `Skipped { needs confirmation }` event, the cloud
    /// approves the specific policy, and the client replays it here.
    pub async fn handle_confirmed(
        &self,
        symptom: Symptom,
        confirmation: Confirmation,
    ) -> HealingEvent {
        self.run(symptom, Some(confirmation)).await
    }

    async fn run(&self, symptom: Symptom, confirmation: Option<Confirmation>) -> HealingEvent {
        let now = Utc::now();
        let id = Uuid::new_v4();

        // Run diagnosis. A healer that returns Err is recorded as a
        // Failed event so the audit trail isn't silent.
        let selected = match self.select(&symptom).await {
            Ok(s) => s,
            Err(e) => {
                let event = HealingEvent {
                    id,
                    at: now,
                    symptom,
                    // Stand-in policy — no remediation chosen. The
                    // Failed outcome is the load-bearing field.
                    policy: HealingPolicy::EscalateToHuman {
                        reason: "diagnose failed".into(),
                    },
                    outcome: HealingOutcome::Failed {
                        error: format!("diagnose: {e}"),
                    },
                };
                self.ledger.record(event.clone());
                return event;
            }
        };

        let Some((winner_idx, diagnosis)) = selected else {
            // No healer recognized the symptom — record so operators
            // see the symptom did surface, even without a remediation.
            let event = HealingEvent {
                id,
                at: now,
                symptom,
                policy: HealingPolicy::EscalateToHuman {
                    reason: "no healer matched".into(),
                },
                outcome: HealingOutcome::Skipped {
                    reason: "inconclusive".into(),
                },
            };
            self.ledger.record(event.clone());
            return event;
        };

        // Destructive policy → never auto-apply on the client. Apply only
        // when a confirmation approves this exact diagnosed policy.
        if diagnosis.recommended.is_destructive() {
            let confirmed = matches!(&confirmation, Some(c) if c.approved == diagnosis.recommended);
            if !confirmed {
                let reason = match &confirmation {
                    Some(_) => "destructive: confirmation did not match diagnosis",
                    None => "destructive: needs cloud confirmation",
                };
                let event = HealingEvent {
                    id,
                    at: now,
                    symptom,
                    policy: diagnosis.recommended,
                    outcome: HealingOutcome::Skipped {
                        reason: reason.into(),
                    },
                };
                self.ledger.record(event.clone());
                return event;
            }
            // Confirmed → fall through to apply.
        }

        // Safe policy (or a confirmed destructive one) → apply via the
        // winning healer (each healer is the only one that knows how to
        // execute its own diagnosis).
        let outcome = match self.healers[winner_idx].apply(&diagnosis.recommended).await {
            Ok(detail) => {
                let detail = match &confirmation {
                    Some(c) => format!("{detail} (confirmed by {})", c.confirmed_by),
                    None => detail,
                };
                HealingOutcome::Applied { detail }
            }
            Err(e) => HealingOutcome::Failed {
                error: format!("apply: {e}"),
            },
        };

        let event = HealingEvent {
            id,
            at: now,
            symptom,
            policy: diagnosis.recommended,
            outcome,
        };
        self.ledger.record(event.clone());
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A healer whose diagnose() and apply() behavior is configured
    /// at construction. Lets us script every scenario the dispatcher
    /// needs to handle without writing a new mock struct per test.
    struct ScriptedHealer {
        recognizes: Option<&'static str>,
        confidence: f32,
        policy: HealingPolicy,
        apply_succeeds: bool,
        diagnose_errors: bool,
        applied: Arc<AtomicUsize>,
    }

    impl ScriptedHealer {
        fn new(
            recognizes: Option<&'static str>,
            confidence: f32,
            policy: HealingPolicy,
        ) -> (Self, Arc<AtomicUsize>) {
            let applied = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    recognizes,
                    confidence,
                    policy,
                    apply_succeeds: true,
                    diagnose_errors: false,
                    applied: applied.clone(),
                },
                applied,
            )
        }

        fn failing_apply(mut self) -> Self {
            self.apply_succeeds = false;
            self
        }

        fn failing_diagnose(mut self) -> Self {
            self.diagnose_errors = true;
            self
        }
    }

    #[async_trait]
    impl Healer for ScriptedHealer {
        async fn diagnose(&self, symptom: &Symptom) -> Result<Option<Diagnosis>, HealerError> {
            if self.diagnose_errors {
                return Err(HealerError::Inconclusive);
            }
            match self.recognizes {
                Some(kind) if kind == symptom.kind => Ok(Some(Diagnosis {
                    symptom: symptom.clone(),
                    rationale: format!("scripted recognized {kind}"),
                    confidence: self.confidence,
                    recommended: self.policy.clone(),
                })),
                _ => Ok(None),
            }
        }

        async fn apply(&self, _policy: &HealingPolicy) -> Result<String, HealerError> {
            self.applied.fetch_add(1, Ordering::SeqCst);
            if self.apply_succeeds {
                Ok("scripted-apply-ok".into())
            } else {
                Err(HealerError::NotImplemented("scripted-apply-fail"))
            }
        }
    }

    fn symptom(kind: &str) -> Symptom {
        Symptom {
            source: "test".into(),
            kind: kind.into(),
            detail: json!({}),
            severity: 1,
        }
    }

    #[tokio::test]
    async fn single_matching_healer_applies_and_records() {
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, applied) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.8,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h));

        let event = d.handle(symptom("sync.stuck")).await;
        assert!(matches!(event.outcome, HealingOutcome::Applied { .. }));
        assert_eq!(applied.load(Ordering::SeqCst), 1);
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn unknown_symptom_records_inconclusive_without_applying() {
        // The dispatcher must still write an audit row so operators
        // see the symptom surfaced — silently dropping it would
        // hide bugs in the source subsystem.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, applied) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.8,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h));

        let event = d.handle(symptom("opcua.disconnect")).await;
        assert!(
            matches!(event.outcome, HealingOutcome::Skipped { ref reason } if reason == "inconclusive")
        );
        assert_eq!(applied.load(Ordering::SeqCst), 0);
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn highest_confidence_wins_when_multiple_match() {
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());

        let (low, applied_low) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.4,
            HealingPolicy::RestartService { name: "low".into() },
        );
        let (high, applied_high) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.9,
            HealingPolicy::RestartService {
                name: "high".into(),
            },
        );
        d.add(Arc::new(low));
        d.add(Arc::new(high));

        let event = d.handle(symptom("sync.stuck")).await;
        // High-confidence healer applied; low-confidence did not.
        assert_eq!(applied_low.load(Ordering::SeqCst), 0);
        assert_eq!(applied_high.load(Ordering::SeqCst), 1);
        assert!(matches!(
            event.policy,
            HealingPolicy::RestartService { ref name } if name == "high"
        ));
    }

    #[tokio::test]
    async fn ties_break_by_insertion_order() {
        // Determinism: when two healers report identical confidence,
        // the one added first wins. Without this we'd get flaky
        // healing outcomes across boots.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());

        let (first, _) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.7,
            HealingPolicy::RestartService {
                name: "first".into(),
            },
        );
        let (second, _) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.7,
            HealingPolicy::RestartService {
                name: "second".into(),
            },
        );
        d.add(Arc::new(first));
        d.add(Arc::new(second));

        let event = d.handle(symptom("sync.stuck")).await;
        assert!(matches!(
            event.policy,
            HealingPolicy::RestartService { ref name } if name == "first"
        ));
    }

    #[tokio::test]
    async fn confidence_floor_filters_low_signals() {
        // A 0.05 diagnosis is essentially "I noticed but I'm not
        // sure" — apply could do more harm than good. Dispatcher
        // treats it as no match.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, applied) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.05,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h));

        let event = d.handle(symptom("sync.stuck")).await;
        assert!(matches!(event.outcome, HealingOutcome::Skipped { .. }));
        assert_eq!(applied.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn destructive_policy_is_skipped_not_applied() {
        // The whole purpose of `is_destructive` is to gate client-side
        // auto-remediation. TrimTelemetry / RecoverSqlite need cloud
        // confirmation — dispatcher must not call apply() on the
        // healer.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, applied) = ScriptedHealer::new(
            Some("telemetry.overflow"),
            0.95,
            HealingPolicy::TrimTelemetry {
                keep_recent_hours: 24,
            },
        );
        d.add(Arc::new(h));

        let event = d.handle(symptom("telemetry.overflow")).await;
        assert!(
            matches!(event.outcome, HealingOutcome::Skipped { ref reason } if reason.contains("destructive"))
        );
        assert_eq!(
            applied.load(Ordering::SeqCst),
            0,
            "destructive apply must not run on the client"
        );
        // But the policy is recorded so operators see what WOULD have
        // been applied if confirmed.
        assert!(matches!(event.policy, HealingPolicy::TrimTelemetry { .. }));
    }

    #[tokio::test]
    async fn confirmed_destructive_policy_is_applied() {
        // Second half of the flow: a cloud confirmation approving the
        // exact diagnosed policy lets the destructive remediation run.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let policy = HealingPolicy::TrimTelemetry {
            keep_recent_hours: 24,
        };
        let (h, applied) = ScriptedHealer::new(Some("telemetry.overflow"), 0.95, policy.clone());
        d.add(Arc::new(h));

        let event = d
            .handle_confirmed(
                symptom("telemetry.overflow"),
                Confirmation {
                    approved: policy,
                    confirmed_by: "edge-fn".into(),
                },
            )
            .await;
        assert!(
            matches!(event.outcome, HealingOutcome::Applied { ref detail } if detail.contains("confirmed by edge-fn"))
        );
        assert_eq!(applied.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mismatched_confirmation_is_rejected() {
        // A confirmation for a DIFFERENT policy (stale / replayed) must
        // not apply the diagnosed one.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, applied) = ScriptedHealer::new(
            Some("telemetry.overflow"),
            0.95,
            HealingPolicy::TrimTelemetry {
                keep_recent_hours: 24,
            },
        );
        d.add(Arc::new(h));

        let event = d
            .handle_confirmed(
                symptom("telemetry.overflow"),
                Confirmation {
                    // Approves a different retention window than diagnosed.
                    approved: HealingPolicy::TrimTelemetry {
                        keep_recent_hours: 999,
                    },
                    confirmed_by: "edge-fn".into(),
                },
            )
            .await;
        assert!(
            matches!(event.outcome, HealingOutcome::Skipped { ref reason } if reason.contains("did not match"))
        );
        assert_eq!(applied.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn apply_failure_is_recorded_as_failed_event() {
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, _) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.9,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h.failing_apply()));

        let event = d.handle(symptom("sync.stuck")).await;
        assert!(
            matches!(event.outcome, HealingOutcome::Failed { ref error } if error.contains("apply"))
        );
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn diagnose_error_records_failed_event() {
        // A healer that crashes mid-diagnose shouldn't take down the
        // whole pipeline silently — record the failure and move on.
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, _) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.9,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h.failing_diagnose()));

        let event = d.handle(symptom("sync.stuck")).await;
        assert!(matches!(event.outcome, HealingOutcome::Failed { .. }));
        assert_eq!(ledger.len(), 1);
    }

    #[tokio::test]
    async fn ledger_accumulates_across_handle_calls() {
        let ledger = Arc::new(HealingLedger::new());
        let mut d = Dispatcher::new(ledger.clone());
        let (h, _) = ScriptedHealer::new(
            Some("sync.stuck"),
            0.9,
            HealingPolicy::RestartService {
                name: "sync".into(),
            },
        );
        d.add(Arc::new(h));

        for _ in 0..3 {
            d.handle(symptom("sync.stuck")).await;
        }
        assert_eq!(ledger.len(), 3);
    }

    #[test]
    fn empty_dispatcher_has_zero_healers() {
        let ledger = Arc::new(HealingLedger::new());
        let d = Dispatcher::new(ledger);
        assert_eq!(d.healer_count(), 0);
    }
}
