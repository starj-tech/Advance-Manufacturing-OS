//! `ScanPipeline` — orchestrator that turns "scanner dispatch a
//! Scan command" into a full audit-logged scan + handle cycle.
//!
//! ## What the pipeline does
//! 1. Dispatches the `Scan` command to the configured scanner
//!    through the V1 `Actuator` trait. Consumes the permit (one
//!    scan per permit; matches the V1 single-use contract).
//! 2. Reads the freshly-recorded payload via `Scanner::last_scan`.
//!    The scanner's dispatch impl is responsible for updating
//!    that snapshot; if it didn't (a scanner-impl bug), the
//!    pipeline surfaces `PipelineError::ScanMissing`.
//! 3. Invokes `ScanHandler::handle` against the payload.
//! 4. Records a `ScanEvent` to the `ScanLedger` regardless of the
//!    handler's verdict — Acknowledge / Reject / Trigger all
//!    produce audit rows.
//! 5. Returns a `ScanResult` with the payload, outcome, and the
//!    event for any post-processing.
//!
//! ## What the pipeline does NOT do
//! Authorization. A handler that returns `Acknowledge` is saying
//! "this scan is recognized" — whether the system should DO
//! anything is the handler's call, not the pipeline's. The
//! pipeline's only contract is "every scan produces a ledger row."
//!
//! Persistence. The `ScanLedger` is in-memory by design (mirrors
//! `InspectionLedger` from V4). Production wires this to the
//! `actuator_commands` table via the V10 sync helper; the
//! pipeline returns events ready for that hand-off.
//!
//! ## Error ordering
//! `Dispatch` (scanner refused the command) → caller retries or
//! re-gates. `ScanMissing` → scanner impl bug, not retryable.
//! `Handler` → infrastructure failure or shape mismatch; the
//! handler error variant carries the kind.

use crate::handler::{HandlerError, HandlerOutcome, ScanHandler};
use crate::scan::{ScanClass, ScannedPayload};
use crate::scanner::Scanner;
use aether_actuators::actuator::ActuatorError;
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::ScannerId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PipelineError {
    /// Scanner's `dispatch` returned an error — typically a permit
    /// failure (consumed, expired) or transport problem. The
    /// contained variant carries the kind for branching.
    #[error("dispatch: {0}")]
    Dispatch(#[from] ActuatorError),
    /// The scanner accepted the Scan command but `last_scan()`
    /// returned None afterwards. That's a scanner-impl contract
    /// violation — every successful Scan MUST update the rolling
    /// snapshot. Surface explicitly rather than silently producing
    /// an empty ledger row.
    #[error("scanner {scanner_id} accepted Scan but didn't record a payload")]
    ScanMissing { scanner_id: ScannerId },
    /// Handler errored. Distinct from "handler returned Reject" —
    /// rejection is a normal verdict and still writes a ledger
    /// row; an error means the handler couldn't complete its
    /// lookup (infrastructure or malformed payload).
    #[error("handler: {0}")]
    Handler(#[from] HandlerError),
}

/// One audit-row's worth of scan metadata. Mirrors the columns the
/// V10 `actuator_commands.result` payload will store — pure data,
/// no behavior. `payload_byte_len` is recorded rather than the raw
/// bytes so the ledger stays small; the bytes themselves live on
/// the result side and (for sensitive classes) get sealed before
/// persistence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanEvent {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    pub scanner_id: ScannerId,
    pub class: ScanClass,
    pub payload_byte_len: usize,
    pub handler_name: String,
    /// Kebab-case slug of the handler's action (`acknowledge` /
    /// `reject` / `trigger`). The full structured action lives on
    /// the `ScanResult.outcome` returned to the caller; the ledger
    /// keeps just the slug so it's filterable without re-parsing.
    pub action_slug: String,
    pub summary: String,
}

/// What `process()` returns to the caller. The pipeline already
/// recorded the event to the ledger; this struct exposes the
/// pieces a post-processor needs (V10 sync layer reads
/// `event` + `outcome.detail` to build the
/// `actuator_commands.result` JSON).
#[derive(Clone, Debug)]
pub struct ScanResult {
    pub event: ScanEvent,
    pub payload: ScannedPayload,
    pub outcome: HandlerOutcome,
}

/// Append-only in-memory ledger. Same shape as
/// `aether_vision::InspectionLedger` (V4) and
/// `aether_healing::HealingLedger` — production swaps the backing
/// store for a Supabase row writer without changing this surface.
#[derive(Default)]
pub struct ScanLedger {
    events: Mutex<Vec<ScanEvent>>,
}

impl ScanLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn record(&self, event: ScanEvent) {
        self.events.lock().await.push(event);
    }

    pub async fn snapshot(&self) -> Vec<ScanEvent> {
        self.events.lock().await.clone()
    }

    pub async fn len(&self) -> usize {
        self.events.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

/// Orchestrator. Cheap to construct; the two trait objects are
/// `Arc`-shared so the same scanner can drive multiple pipelines
/// with different handlers (e.g. one pipeline for badge unlock,
/// another for lot consumption, both reading the same RFID antenna
/// at different shifts).
pub struct ScanPipeline {
    scanner: Arc<dyn Scanner>,
    handler: Arc<dyn ScanHandler>,
    ledger: Arc<ScanLedger>,
}

impl ScanPipeline {
    pub fn new(
        scanner: Arc<dyn Scanner>,
        handler: Arc<dyn ScanHandler>,
        ledger: Arc<ScanLedger>,
    ) -> Self {
        Self {
            scanner,
            handler,
            ledger,
        }
    }

    pub fn ledger(&self) -> &Arc<ScanLedger> {
        &self.ledger
    }

    /// Run one full scan: dispatch → retrieve payload → handle →
    /// record → return.
    ///
    /// `cmd` should be an `ActuatorCommand::Scan { trigger }`;
    /// other variants pass through to the scanner and are
    /// rejected there (which the pipeline surfaces as `Dispatch`).
    /// We don't pre-filter here because passing a Halt through
    /// the pipeline is a legitimate "scan-then-stop" workflow
    /// step in continuous-mode setups.
    pub async fn process(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ScanResult, PipelineError> {
        // 1. Dispatch through the V1 actuator path.
        self.scanner.dispatch(cmd, permit).await?;

        // 2. Read the freshly-recorded payload. The scanner is
        // responsible for updating `last_scan` on a successful
        // Scan; we rely on that contract here.
        let payload = self
            .scanner
            .last_scan()
            .await
            .ok_or_else(|| PipelineError::ScanMissing {
                scanner_id: self.scanner.scanner_id(),
            })?;

        // 3. Invoke the handler. Infrastructure errors propagate;
        // typed rejections (Reject variant) are NOT errors and
        // continue to step 4.
        let outcome = self.handler.handle(&payload).await?;

        // 4. Build the event and record it. Every verdict —
        // Acknowledge / Reject / Trigger — produces an audit row.
        let event = ScanEvent {
            id: Uuid::now_v7(),
            at: Utc::now(),
            scanner_id: self.scanner.scanner_id(),
            class: payload.class,
            payload_byte_len: payload.bytes.len(),
            handler_name: self.handler.handler_name().to_string(),
            action_slug: outcome.action.slug().to_string(),
            summary: outcome.summary.clone(),
        };
        self.ledger.record(event.clone()).await;

        // 5. Return the assembled result. Caller is the V10 sync
        // helper or a test assertion.
        Ok(ScanResult {
            event,
            payload,
            outcome,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::HandlerAction;
    use crate::mock::MockScanner;
    use crate::scan::ScanClass;
    use aether_actuators::command::ScanTrigger;
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use async_trait::async_trait;
    use chrono::Duration;
    use std::sync::atomic::{AtomicUsize, Ordering};
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

    /// Canned-outcome handler. Each call pops the next configured
    /// response (FIFO), or returns `Acknowledge` if the queue is
    /// empty. Counts calls so tests can assert how many times the
    /// pipeline reached the handler.
    struct MockHandler {
        name: &'static str,
        responses: Mutex<Vec<Result<HandlerOutcome, HandlerError>>>,
        calls: AtomicUsize,
    }

    impl MockHandler {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                responses: Mutex::new(Vec::new()),
                calls: AtomicUsize::new(0),
            }
        }

        async fn enqueue(&self, r: Result<HandlerOutcome, HandlerError>) {
            self.responses.lock().await.push(r);
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ScanHandler for MockHandler {
        fn handler_name(&self) -> &'static str {
            self.name
        }
        async fn handle(&self, _payload: &ScannedPayload) -> Result<HandlerOutcome, HandlerError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut q = self.responses.lock().await;
            if q.is_empty() {
                Ok(HandlerOutcome {
                    action: HandlerAction::Acknowledge,
                    summary: "default-ack".into(),
                    detail: serde_json::json!({}),
                })
            } else {
                q.remove(0)
            }
        }
    }

    async fn fixture_scanner_with_payload(class: ScanClass, bytes: &[u8]) -> Arc<MockScanner> {
        let s = Arc::new(MockScanner::new());
        let id = s.scanner_id();
        s.enqueue_scan(ScannedPayload {
            scanner_id: id,
            read_at: Utc::now(),
            class,
            bytes: bytes.to_vec(),
            confidence: Some(0.9),
        })
        .await
        .unwrap();
        s
    }

    #[tokio::test]
    async fn happy_path_acknowledge_writes_one_ledger_row_and_returns_outcome() {
        let scanner = fixture_scanner_with_payload(ScanClass::QrCode, b"LOT-7").await;
        let handler = Arc::new(MockHandler::new("test-handler"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner.clone(), handler.clone(), ledger.clone());

        let result = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.outcome.action, HandlerAction::Acknowledge);
        assert_eq!(result.event.handler_name, "test-handler");
        assert_eq!(result.event.action_slug, "acknowledge");
        assert_eq!(result.event.class, ScanClass::QrCode);
        assert_eq!(result.payload.bytes, b"LOT-7");
        assert_eq!(handler.call_count(), 1);
        assert_eq!(ledger.len().await, 1);
    }

    #[tokio::test]
    async fn reject_outcome_still_writes_a_ledger_row() {
        // Reject is a NORMAL verdict; pipeline records it the same
        // way as Acknowledge. Tests "not an error" semantics.
        let scanner = fixture_scanner_with_payload(ScanClass::RfidIso14443a, &[0xDE, 0xAD]).await;
        let handler = Arc::new(MockHandler::new("badge-handler"));
        handler
            .enqueue(Ok(HandlerOutcome {
                action: HandlerAction::Reject {
                    reason: "uid not in allowlist".into(),
                },
                summary: "badge denied".into(),
                detail: serde_json::json!({"uid": "deadbeef"}),
            }))
            .await;
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger.clone());

        let result = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Auto,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert!(matches!(
            result.outcome.action,
            HandlerAction::Reject { .. }
        ));
        assert_eq!(result.event.action_slug, "reject");
        assert_eq!(ledger.len().await, 1);
        assert_eq!(ledger.snapshot().await[0].summary, "badge denied");
    }

    #[tokio::test]
    async fn trigger_outcome_carries_ref_id_into_outcome_but_not_event() {
        // The full structured action (target + ref_id) stays on
        // the ScanResult.outcome side; the ledger keeps just the
        // slug for filterability. Pin that contract.
        let scanner =
            fixture_scanner_with_payload(ScanClass::Gs1_128, b"01070112345678901721").await;
        let handler = Arc::new(MockHandler::new("dispatch-handler"));
        handler
            .enqueue(Ok(HandlerOutcome {
                action: HandlerAction::Trigger {
                    target: "agv-dispatch".into(),
                    ref_id: Some("route-42".into()),
                },
                summary: "routed".into(),
                detail: serde_json::json!({"route_id": "route-42"}),
            }))
            .await;
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger.clone());

        let result = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.event.action_slug, "trigger");
        // Ref_id lives on the outcome, not the event.
        match result.outcome.action {
            HandlerAction::Trigger { target, ref_id } => {
                assert_eq!(target, "agv-dispatch");
                assert_eq!(ref_id, Some("route-42".into()));
            }
            other => panic!("expected Trigger, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_error_surfaces_as_pipeline_error_and_skips_ledger() {
        // An empty scan queue surfaces as BadCommand from the
        // mock scanner — that's a Dispatch error to the pipeline.
        // The ledger MUST NOT have a row because no scan happened.
        let scanner = Arc::new(MockScanner::new());
        let handler = Arc::new(MockHandler::new("h"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler.clone(), ledger.clone());

        let err = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            PipelineError::Dispatch(ActuatorError::BadCommand(_))
        ));
        // Handler never reached.
        assert_eq!(handler.call_count(), 0);
        assert_eq!(ledger.len().await, 0);
    }

    #[tokio::test]
    async fn handler_infrastructure_error_propagates_and_does_not_write_ledger() {
        // Handler error means the lookup couldn't complete. We
        // already burned the scan — but we refuse to write an
        // audit row with an unknown verdict. Caller's job to
        // retry (re-scan with a fresh permit).
        let scanner = fixture_scanner_with_payload(ScanClass::QrCode, b"X").await;
        let handler = Arc::new(MockHandler::new("flaky"));
        handler
            .enqueue(Err(HandlerError::Infrastructure(
                "cert store unreachable".into(),
            )))
            .await;
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger.clone());

        let err = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            PipelineError::Handler(HandlerError::Infrastructure(_))
        ));
        // No partial audit row — this is the V4 InspectionPipeline
        // half-record refusal pattern, applied to scan handling.
        assert_eq!(ledger.len().await, 0);
    }

    #[tokio::test]
    async fn payload_byte_len_in_event_matches_actual_payload() {
        // Tests pin the data contract: the event records the
        // length of the bytes that the scanner returned, which is
        // what the V10 outbox encoder will reference when sizing
        // its result.bytes column.
        let scanner = fixture_scanner_with_payload(ScanClass::Code128, b"123-456-789").await;
        let handler = Arc::new(MockHandler::new("h"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger);

        let r = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(r.event.payload_byte_len, b"123-456-789".len());
    }

    #[tokio::test]
    async fn scanner_id_in_event_matches_underlying_scanner() {
        // The event's scanner_id is the source-of-truth for joins
        // with the V10 actuator_commands rows. Pin equality.
        let scanner = fixture_scanner_with_payload(ScanClass::QrCode, b"X").await;
        let expected_id = scanner.scanner_id();
        let handler = Arc::new(MockHandler::new("h"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger);

        let r = pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(r.event.scanner_id, expected_id);
    }

    #[tokio::test]
    async fn ledger_snapshot_is_independent_of_subsequent_writes() {
        // The ledger's snapshot is by-clone — once a test grabs
        // it, subsequent pipeline runs don't retroactively appear
        // in the captured Vec. Mirrors InspectionLedger semantics.
        let scanner = Arc::new(MockScanner::new());
        let id = scanner.scanner_id();
        for i in 0..3 {
            scanner
                .enqueue_scan(ScannedPayload {
                    scanner_id: id,
                    read_at: Utc::now(),
                    class: ScanClass::QrCode,
                    bytes: vec![i],
                    confidence: None,
                })
                .await
                .unwrap();
        }
        let handler = Arc::new(MockHandler::new("h"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger.clone());

        pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        let snapshot_after_one = ledger.snapshot().await;
        assert_eq!(snapshot_after_one.len(), 1);

        pipeline
            .process(
                ActuatorCommand::Scan {
                    trigger: ScanTrigger::Manual,
                },
                test_permit(),
            )
            .await
            .unwrap();
        // The earlier snapshot didn't grow.
        assert_eq!(snapshot_after_one.len(), 1);
        // The ledger itself did.
        assert_eq!(ledger.len().await, 2);
    }

    #[tokio::test]
    async fn three_scans_produce_three_independent_event_ids() {
        // UUIDv7 ensures monotonic ordering AND uniqueness across
        // rapid-succession scans. Pin both.
        let scanner = Arc::new(MockScanner::new());
        let id = scanner.scanner_id();
        for _ in 0..3 {
            scanner
                .enqueue_scan(ScannedPayload {
                    scanner_id: id,
                    read_at: Utc::now(),
                    class: ScanClass::QrCode,
                    bytes: b"X".to_vec(),
                    confidence: None,
                })
                .await
                .unwrap();
        }
        let handler = Arc::new(MockHandler::new("h"));
        let ledger = Arc::new(ScanLedger::new());
        let pipeline = ScanPipeline::new(scanner, handler, ledger.clone());

        for _ in 0..3 {
            pipeline
                .process(
                    ActuatorCommand::Scan {
                        trigger: ScanTrigger::Manual,
                    },
                    test_permit(),
                )
                .await
                .unwrap();
        }
        let events = ledger.snapshot().await;
        assert_eq!(events.len(), 3);
        assert_ne!(events[0].id, events[1].id);
        assert_ne!(events[1].id, events[2].id);
        // UUIDv7 is time-ordered — later ids are >= earlier ids.
        assert!(events[0].id <= events[1].id);
        assert!(events[1].id <= events[2].id);
    }
}
