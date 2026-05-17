//! [`MockScanner`] — canned scan queue with FIFO consumption.
//!
//! Pipeline tests load a sequence of scans up-front (one badge
//! tap, two lot scans, one Halt), then dispatch `Scan` commands
//! and assert the rolling state matches. Real RFID/barcode
//! hardware isn't needed for the workflow logic itself — that
//! lives in V12.
//!
//! ## Trigger contract
//! Each Scan command pops the next queued payload regardless of
//! the trigger (Manual / Auto / Continuous). The trigger is
//! recorded into the audit row so a downstream test can assert
//! "the workflow code dispatched with the right trigger" without
//! the mock conflating the cases.
//!
//! ## What an empty queue does
//! Returns `ActuatorError::BadCommand("scan queue empty")` so
//! tests that exhaust the queue surface as a typed failure
//! rather than silently returning a stale or default payload.
//! Stale-payload semantics belong to the real-hardware impl
//! (continuous-mode antennas keep returning the last tag seen);
//! the mock is for canned scenarios.

use crate::scan::ScannedPayload;
use crate::scanner::Scanner;
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::{ActuatorCommand, ScanTrigger};
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, ScannerId};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::VecDeque;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum ScanQueueError {
    /// `enqueue_scan` was called with a payload whose
    /// `scanner_id` doesn't match this mock's. Catches the
    /// common test-fixture bug of constructing payloads with a
    /// fresh ID and forgetting to wire it to the scanner.
    #[error("payload scanner_id does not match this MockScanner ({0} != {1})")]
    IdMismatch(ScannerId, ScannerId),
}

pub struct MockScanner {
    actuator_id: ActuatorId,
    scanner_id: ScannerId,
    queued: Mutex<VecDeque<ScannedPayload>>,
    last: Mutex<Option<ScannedPayload>>,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockScanner {
    pub fn new() -> Self {
        Self {
            actuator_id: ActuatorId::new(),
            scanner_id: ScannerId::new(),
            queued: Mutex::new(VecDeque::new()),
            last: Mutex::new(None),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a scan payload to be returned on the next Scan
    /// dispatch. Returns `IdMismatch` if the payload's scanner_id
    /// doesn't match — that's a test-fixture bug worth surfacing
    /// loudly rather than silently propagating a wrong ID
    /// through the audit row.
    pub async fn enqueue_scan(&self, payload: ScannedPayload) -> Result<(), ScanQueueError> {
        if payload.scanner_id != self.scanner_id {
            return Err(ScanQueueError::IdMismatch(
                payload.scanner_id,
                self.scanner_id,
            ));
        }
        self.queued.lock().await.push_back(payload);
        Ok(())
    }

    /// Number of payloads waiting to be popped. Useful for
    /// asserting "test consumed exactly N scans."
    pub async fn queued_count(&self) -> usize {
        self.queued.lock().await.len()
    }

    /// Snapshot the commands received so far. Mirrors the
    /// `MockCamera`/`MockArm`/`MockAgv` pattern — rejected
    /// commands also appear here so audit assertions are
    /// uniform.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }
}

impl Default for MockScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Actuator for MockScanner {
    fn actuator_id(&self) -> ActuatorId {
        self.actuator_id
    }
    fn actuator_kind(&self) -> &'static str {
        "scanner"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::Scan { trigger } => {
                let payload = self
                    .queued
                    .lock()
                    .await
                    .pop_front()
                    .ok_or_else(|| ActuatorError::BadCommand("scan queue empty".into()))?;
                let data = serde_json::json!({
                    "class": payload.class.slug(),
                    "bytes_len": payload.bytes.len(),
                    "confidence": payload.confidence,
                    "trigger": match trigger {
                        ScanTrigger::Manual => "manual",
                        ScanTrigger::Auto => "auto",
                        ScanTrigger::Continuous => "continuous",
                    },
                });
                *self.last.lock().await = Some(payload);
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "scan".into(),
                    completed_at: Utc::now(),
                    data,
                })
            }
            ActuatorCommand::Halt => {
                // Halt clears the latest snapshot so a subsequent
                // last_scan() returns None until a fresh dispatch
                // pulls another payload off the queue. Real
                // industrial readers turn off antenna power on
                // Halt; this mirrors that observable.
                *self.last.lock().await = None;
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "scanner does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl Scanner for MockScanner {
    fn scanner_id(&self) -> ScannerId {
        self.scanner_id
    }
    async fn last_scan(&self) -> Option<ScannedPayload> {
        self.last.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::ScanClass;
    use aether_actuators::actuator::ActuatorError;
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
                code: "shop-floor".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["shop-floor".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock approves in test fixture")
    }

    fn fixture_payload(scanner_id: ScannerId, class: ScanClass, bytes: &[u8]) -> ScannedPayload {
        ScannedPayload {
            scanner_id,
            read_at: Utc::now(),
            class,
            bytes: bytes.to_vec(),
            confidence: Some(0.95),
        }
    }

    #[tokio::test]
    async fn fresh_scanner_has_no_last_scan() {
        let s = MockScanner::new();
        assert!(s.last_scan().await.is_none());
    }

    #[tokio::test]
    async fn enqueue_then_dispatch_returns_payload_in_fifo_order() {
        let s = MockScanner::new();
        let id = s.scanner_id();
        s.enqueue_scan(fixture_payload(id, ScanClass::QrCode, b"LOT-A"))
            .await
            .unwrap();
        s.enqueue_scan(fixture_payload(id, ScanClass::QrCode, b"LOT-B"))
            .await
            .unwrap();

        let result = s
            .dispatch(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.command_kind, "scan");
        // last_scan reflects the first popped payload.
        let last = s.last_scan().await.unwrap();
        assert_eq!(last.bytes, b"LOT-A");

        // Second dispatch pops the second one.
        s.dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Manual,
            },
            test_permit(),
        )
        .await
        .unwrap();
        let last = s.last_scan().await.unwrap();
        assert_eq!(last.bytes, b"LOT-B");
    }

    #[tokio::test]
    async fn enqueue_rejects_payload_with_mismatched_scanner_id() {
        // Common test-fixture footgun: build a payload with a
        // fresh ScannerId and forget to thread the scanner's
        // actual id through. Surface as typed error so the test
        // fails clearly rather than silently mis-routing.
        let s = MockScanner::new();
        let wrong = fixture_payload(ScannerId::new(), ScanClass::QrCode, b"X");
        let err = s.enqueue_scan(wrong).await.unwrap_err();
        assert!(matches!(err, ScanQueueError::IdMismatch(_, _)));
    }

    #[tokio::test]
    async fn dispatch_against_empty_queue_surfaces_bad_command() {
        // No silent stale-payload semantics — the mock is for
        // canned tests, and an empty queue is a test bug.
        let s = MockScanner::new();
        let err = s
            .dispatch(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn halt_clears_last_scan_and_queue_is_untouched() {
        // After Halt, last_scan returns None — real antenna
        // power-off semantic. The queue isn't drained; tests
        // that re-arm after a Halt should still see queued
        // payloads available.
        let s = MockScanner::new();
        let id = s.scanner_id();
        s.enqueue_scan(fixture_payload(id, ScanClass::Code128, b"A"))
            .await
            .unwrap();
        s.dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Manual,
            },
            test_permit(),
        )
        .await
        .unwrap();
        assert!(s.last_scan().await.is_some());

        s.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert!(s.last_scan().await.is_none());

        // Queue is untouched — queue carries pending reads, Halt
        // only affects the rolling snapshot.
        s.enqueue_scan(fixture_payload(id, ScanClass::Code128, b"B"))
            .await
            .unwrap();
        assert_eq!(s.queued_count().await, 1);
    }

    #[tokio::test]
    async fn non_scan_non_halt_commands_are_rejected_with_bad_command() {
        // Scanners reject MoveJoint / MoveLinear / DispatchJob /
        // Capture — same shape as camera/arm/agv rejecting the
        // commands they don't recognize.
        let s = MockScanner::new();
        let err = s
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

        let err = s
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 80,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn dispatched_and_rejected_commands_all_appear_in_received_log() {
        // Audit-uniformity: rejected commands MUST also appear in
        // the received log so an investigator can see what was
        // attempted.
        let s = MockScanner::new();
        let id = s.scanner_id();
        s.enqueue_scan(fixture_payload(id, ScanClass::QrCode, b"OK"))
            .await
            .unwrap();
        // One accepted, one rejected (MoveJoint), one Halt.
        s.dispatch(
            ActuatorCommand::Scan {
                trigger: ScanTrigger::Auto,
            },
            test_permit(),
        )
        .await
        .unwrap();
        let _ = s
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 0.0,
                },
                test_permit(),
            )
            .await;
        s.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();

        let log = s.received().await;
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].kind(), CommandKind::Scan);
        assert_eq!(log[1].kind(), CommandKind::MoveJoint);
        assert_eq!(log[2].kind(), CommandKind::Halt);
    }

    #[tokio::test]
    async fn trigger_kind_propagates_into_result_data() {
        // The audit row carries the trigger so downstream
        // analytics can distinguish "operator pulled the
        // trigger" from "workflow auto-dispatched the scan."
        // Pin that the result.data carries the trigger string.
        let s = MockScanner::new();
        let id = s.scanner_id();
        s.enqueue_scan(fixture_payload(id, ScanClass::QrCode, b"X"))
            .await
            .unwrap();
        let result = s
            .dispatch(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Continuous,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.data["trigger"], "continuous");
    }

    #[tokio::test]
    async fn scanner_id_distinct_from_actuator_id() {
        // The two IDs share a UUID kind but live in distinct
        // newtypes — cross-table joins on scans.scanner_id MUST
        // use ScannerId, not ActuatorId, to keep the schema
        // self-checking.
        let s = MockScanner::new();
        // Both UUIDs are minted independently so they have
        // distinct values; the contract is that joins use the
        // type-correct projection.
        assert_ne!(s.scanner_id().0, s.actuator_id().0);
    }
}
