//! [`MockHmi`] — in-memory event queue + announcement log.
//!
//! ## Behavior
//! - `enqueue_event` pushes an `OperatorEvent` into the pending
//!   buffer. Real surfaces would emit these from gesture
//!   recognition / ASR / hardware-button drivers; the mock lets
//!   tests synthesize them deterministically.
//! - `pending_events` drains the buffer FIFO and clears it.
//!   Repeated calls without new enqueues return empty Vecs —
//!   the drain semantic prevents the audit ledger from double-
//!   counting.
//! - `announcements` snapshots the announcements that have been
//!   dispatched, so tests can assert "the workflow pushed
//!   exactly N critical alarms."
//!
//! ## Voice / gaze guardrail
//! `enqueue_event` rejects Voice / GazeDwell events against a
//! surface whose `HmiKind` doesn't support them. Catches the
//! test-fixture bug of queuing a Voice utterance against a
//! Touchscreen — same shape of guardrail as V11's MockScanner
//! ID-mismatch refusal.

use crate::event::OperatorEvent;
use crate::hmi::Hmi;
use crate::kind::HmiKind;
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::{ActuatorCommand, AnnounceSeverity};
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, HmiId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum EventQueueError {
    /// Tried to enqueue a Voice event against a surface that
    /// doesn't support voice (Touchscreen, Pendant). The check
    /// catches test-fixture bugs early.
    #[error("surface {kind:?} does not support voice events")]
    UnsupportedVoice { kind: HmiKind },
    /// Tried to enqueue a GazeDwell event against a surface that
    /// doesn't support gaze (everything except SmartGlasses).
    #[error("surface {kind:?} does not support gaze events")]
    UnsupportedGaze { kind: HmiKind },
}

/// Recorded outbound announcement. Captured into the log so
/// tests can assert what was dispatched.
#[derive(Clone, Debug)]
pub struct AnnouncementRecord {
    pub severity: AnnounceSeverity,
    pub summary: String,
    pub body: Option<String>,
    pub at: DateTime<Utc>,
}

pub struct MockHmi {
    actuator_id: ActuatorId,
    hmi_id: HmiId,
    kind: HmiKind,
    pending: Mutex<VecDeque<OperatorEvent>>,
    announcements: Mutex<Vec<AnnouncementRecord>>,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockHmi {
    pub fn new(kind: HmiKind) -> Self {
        Self {
            actuator_id: ActuatorId::new(),
            hmi_id: HmiId::new(),
            kind,
            pending: Mutex::new(VecDeque::new()),
            announcements: Mutex::new(Vec::new()),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue an operator event for the next `pending_events`
    /// drain. Rejects Voice / GazeDwell against incompatible
    /// surfaces (the capability check prevents test-fixture
    /// bugs of synthesizing events the real surface couldn't
    /// emit).
    pub async fn enqueue_event(&self, event: OperatorEvent) -> Result<(), EventQueueError> {
        match &event {
            OperatorEvent::Voice { .. } if !self.kind.supports_voice() => {
                return Err(EventQueueError::UnsupportedVoice { kind: self.kind });
            }
            OperatorEvent::GazeDwell { .. } if !self.kind.supports_gaze() => {
                return Err(EventQueueError::UnsupportedGaze { kind: self.kind });
            }
            _ => {}
        }
        self.pending.lock().await.push_back(event);
        Ok(())
    }

    /// Snapshot the announcements dispatched so far. Tests use
    /// this to assert "the workflow pushed exactly N critical
    /// alarms in the right order."
    pub async fn announcements(&self) -> Vec<AnnouncementRecord> {
        self.announcements.lock().await.clone()
    }

    /// Number of events currently queued. Drops to zero after
    /// each `pending_events` drain.
    pub async fn pending_count(&self) -> usize {
        self.pending.lock().await.len()
    }

    /// Snapshot the commands received so far (audit-uniformity
    /// pattern from V1-V13).
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }
}

#[async_trait]
impl Actuator for MockHmi {
    fn actuator_id(&self) -> ActuatorId {
        self.actuator_id
    }
    fn actuator_kind(&self) -> &'static str {
        "hmi"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::Announce {
                severity,
                summary,
                body,
            } => {
                let record = AnnouncementRecord {
                    severity,
                    summary: summary.clone(),
                    body: body.clone(),
                    at: Utc::now(),
                };
                self.announcements.lock().await.push(record);
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "announce".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({
                        "severity": severity.slug(),
                        "summary": summary,
                        "has_body": body.is_some(),
                    }),
                })
            }
            ActuatorCommand::Halt => {
                // Halt on an HMI clears any pending events (the
                // operator has lost context after a cell halt;
                // surfacing stale taps would mislead the next
                // workflow step). Mirrors real-surface behavior
                // where e-stop blanks the screen.
                self.pending.lock().await.clear();
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "hmi does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl Hmi for MockHmi {
    fn hmi_id(&self) -> HmiId {
        self.hmi_id
    }
    fn hmi_kind(&self) -> HmiKind {
        self.kind
    }
    async fn pending_events(&self) -> Vec<OperatorEvent> {
        // Drain semantic: take everything in the queue and
        // leave it empty so the next call doesn't double-
        // return. Pinned by the
        // `pending_events_drains_buffer_so_repeat_calls_return_empty`
        // test.
        let mut buf = self.pending.lock().await;
        let drained: Vec<_> = buf.drain(..).collect();
        drained
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::SwipeDirection;
    use aether_actuators::command::CommandKind;
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use uuid::Uuid;

    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock approves in test fixture")
    }

    #[tokio::test]
    async fn fresh_hmi_has_no_pending_events_or_announcements() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        assert_eq!(h.pending_count().await, 0);
        assert!(h.pending_events().await.is_empty());
        assert!(h.announcements().await.is_empty());
    }

    #[tokio::test]
    async fn enqueue_then_drain_returns_events_in_fifo_order() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        let now = Utc::now();
        h.enqueue_event(OperatorEvent::Tap {
            region: "a".into(),
            at: now,
        })
        .await
        .unwrap();
        h.enqueue_event(OperatorEvent::Tap {
            region: "b".into(),
            at: now,
        })
        .await
        .unwrap();
        let drained = h.pending_events().await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].slug(), "tap");
        match (&drained[0], &drained[1]) {
            (OperatorEvent::Tap { region: r1, .. }, OperatorEvent::Tap { region: r2, .. }) => {
                assert_eq!(r1, "a");
                assert_eq!(r2, "b");
            }
            _ => panic!("expected two Tap events"),
        }
    }

    #[tokio::test]
    async fn pending_events_drains_buffer_so_repeat_calls_return_empty() {
        // The audit-ledger correctness invariant: each event is
        // delivered exactly once across all `pending_events`
        // drains. A second drain MUST return empty.
        let h = MockHmi::new(HmiKind::Touchscreen);
        h.enqueue_event(OperatorEvent::Acknowledge { at: Utc::now() })
            .await
            .unwrap();
        assert_eq!(h.pending_events().await.len(), 1);
        assert_eq!(h.pending_events().await.len(), 0);
    }

    #[tokio::test]
    async fn voice_event_rejected_on_touchscreen_with_typed_error() {
        // Capability guardrail — Voice against a non-voice
        // surface is a test-fixture bug.
        let h = MockHmi::new(HmiKind::Touchscreen);
        let err = h
            .enqueue_event(OperatorEvent::Voice {
                utterance: "halt".into(),
                confidence: None,
                at: Utc::now(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, EventQueueError::UnsupportedVoice { .. }));
    }

    #[tokio::test]
    async fn voice_event_accepted_on_smart_glasses() {
        let h = MockHmi::new(HmiKind::SmartGlasses);
        h.enqueue_event(OperatorEvent::Voice {
            utterance: "halt".into(),
            confidence: Some(0.9),
            at: Utc::now(),
        })
        .await
        .unwrap();
        assert_eq!(h.pending_count().await, 1);
    }

    #[tokio::test]
    async fn gaze_event_rejected_on_rugged_tablet_with_typed_error() {
        let h = MockHmi::new(HmiKind::RuggedTablet);
        let err = h
            .enqueue_event(OperatorEvent::GazeDwell {
                region: "menu".into(),
                duration_ms: 600,
                at: Utc::now(),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, EventQueueError::UnsupportedGaze { .. }));
    }

    #[tokio::test]
    async fn gaze_event_accepted_on_smart_glasses() {
        let h = MockHmi::new(HmiKind::SmartGlasses);
        h.enqueue_event(OperatorEvent::GazeDwell {
            region: "menu".into(),
            duration_ms: 600,
            at: Utc::now(),
        })
        .await
        .unwrap();
        assert_eq!(h.pending_count().await, 1);
    }

    #[tokio::test]
    async fn announce_dispatch_records_announcement_and_returns_result() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        let result = h
            .dispatch(
                ActuatorCommand::Announce {
                    severity: AnnounceSeverity::Critical,
                    summary: "spindle fault".into(),
                    body: Some("axis Z timeout".into()),
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.command_kind, "announce");
        assert_eq!(result.data["severity"], "critical");
        assert_eq!(result.data["has_body"], true);

        let log = h.announcements().await;
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].severity, AnnounceSeverity::Critical);
        assert_eq!(log[0].summary, "spindle fault");
    }

    #[tokio::test]
    async fn announce_with_no_body_records_none_in_log() {
        let h = MockHmi::new(HmiKind::Pendant);
        h.dispatch(
            ActuatorCommand::Announce {
                severity: AnnounceSeverity::Info,
                summary: "shift change at 14:00".into(),
                body: None,
            },
            test_permit(),
        )
        .await
        .unwrap();
        let log = h.announcements().await;
        assert_eq!(log.len(), 1);
        assert!(log[0].body.is_none());
    }

    #[tokio::test]
    async fn halt_clears_pending_events_so_stale_taps_dont_leak() {
        // Real HMI surfaces blank the screen on e-stop; the mock
        // mirrors that observable so post-halt drains don't
        // return stale taps from before the halt.
        let h = MockHmi::new(HmiKind::Touchscreen);
        h.enqueue_event(OperatorEvent::Tap {
            region: "x".into(),
            at: Utc::now(),
        })
        .await
        .unwrap();
        assert_eq!(h.pending_count().await, 1);

        h.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(h.pending_count().await, 0);
    }

    #[tokio::test]
    async fn halt_does_not_clear_announcement_log() {
        // The announcement log is audit history — it MUST
        // survive a Halt so the operator can review what alerts
        // led up to the stop.
        let h = MockHmi::new(HmiKind::Touchscreen);
        h.dispatch(
            ActuatorCommand::Announce {
                severity: AnnounceSeverity::Critical,
                summary: "x".into(),
                body: None,
            },
            test_permit(),
        )
        .await
        .unwrap();
        h.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(h.announcements().await.len(), 1);
    }

    #[tokio::test]
    async fn non_announce_non_halt_commands_are_rejected() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        let err = h
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 0.0,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn dispatched_and_rejected_commands_all_appear_in_received_log() {
        // Audit-uniformity: rejected commands appear too.
        let h = MockHmi::new(HmiKind::Touchscreen);
        h.dispatch(
            ActuatorCommand::Announce {
                severity: AnnounceSeverity::Info,
                summary: "x".into(),
                body: None,
            },
            test_permit(),
        )
        .await
        .unwrap();
        let _ = h
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 80,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await;
        h.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        let log = h.received().await;
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].kind(), CommandKind::Announce);
        assert_eq!(log[1].kind(), CommandKind::Capture);
        assert_eq!(log[2].kind(), CommandKind::Halt);
    }

    #[tokio::test]
    async fn hmi_kind_is_returned_verbatim_for_every_variant() {
        for k in [
            HmiKind::Touchscreen,
            HmiKind::RuggedTablet,
            HmiKind::SmartGlasses,
            HmiKind::Pendant,
        ] {
            let h = MockHmi::new(k);
            assert_eq!(h.hmi_kind(), k);
        }
    }

    #[tokio::test]
    async fn hmi_id_distinct_from_actuator_id() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        assert_ne!(h.hmi_id().0, h.actuator_id().0);
    }

    #[tokio::test]
    async fn swipe_event_round_trips_through_drain() {
        let h = MockHmi::new(HmiKind::Touchscreen);
        h.enqueue_event(OperatorEvent::Swipe {
            region: "menu".into(),
            direction: SwipeDirection::Left,
            at: Utc::now(),
        })
        .await
        .unwrap();
        let drained = h.pending_events().await;
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].slug(), "swipe");
    }
}
