//! `InspectionPipeline` — the orchestrator that turns "operator
//! presses inspect" into a full audit-logged capture + detect +
//! (optionally) seal cycle.
//!
//! ## What the pipeline does
//! 1. Dispatches `Capture` to the camera through the `Actuator`
//!    trait. Requires a valid `ActuatorPermit` — caller mints it
//!    via `aether_actuators::gate(UnlockRequest)` so the safety
//!    interlock is checked exactly once per inspection.
//! 2. Extracts the `frame_id` from the dispatch result's metadata.
//! 3. Retrieves the captured `Frame` from the camera's rolling
//!    buffer.
//! 4. Runs the `DefectDetector` against the frame.
//! 5. If `frame.privacy.requires_encryption()` is true, seals the
//!    bytes via the configured `FrameSealer`. If no sealer is
//!    configured AND the frame is sensitive, surfaces
//!    `PipelineError::SealerMissing` — refuses to silently leak
//!    a sensitive frame.
//! 6. Records an `InspectionEvent` to the `InspectionLedger`. The
//!    event carries metadata + defect list + sealed-byte-len; the
//!    ledger is the audit trail.
//! 7. Returns an `InspectionResult` with the ready-to-store bytes,
//!    defects, and the recorded event.
//!
//! ## What the pipeline does NOT do
//! Persistence. The `InspectionLedger` is in-memory; production
//! wires this to the `actuator_commands` Supabase table via
//! `aether-db` (a future session). Same pattern as
//! `aether_healing::HealingLedger`. Storing the `stored_bytes`
//! into object storage is also out of scope here — the pipeline
//! returns them; whoever called the pipeline writes.
//!
//! ## Error ordering
//! Failures are surfaced as specific `PipelineError` variants so
//! callers can branch: `Dispatch` → operator might retry; `Inference`
//! → detector misconfig; `Sealing` → cryptographic failure;
//! `SealerMissing` → operational misconfiguration (sensitive
//! camera bound without a sealer). Each variant carries enough
//! context to populate the UI alert.

use crate::camera::Camera;
use crate::frame::FrameId;
use crate::inference::{Defect, DefectDetector, DetectorError};
use crate::privacy::PrivacyClass;
use crate::sealer::{FrameSealer, SealError};
use aether_actuators::actuator::ActuatorError;
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::CameraId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PipelineError {
    /// Camera's `dispatch` returned an error — typically a permit
    /// failure or transport problem. The contained variant carries
    /// the kind so callers can decide retry vs. re-gate vs. alert.
    #[error("dispatch: {0}")]
    Dispatch(#[from] ActuatorError),
    /// Camera accepted the capture but the result's `data` didn't
    /// carry a parseable `frame_id`. Indicates a camera-impl bug
    /// (every camera MUST return one per V2's contract).
    #[error("malformed dispatch result: {0}")]
    BadDispatchResult(String),
    /// Frame fell out of the camera's rolling buffer before we
    /// could retrieve it. Possible if the pipeline got delayed
    /// past `MOCK_BUFFER_CAPACITY` more captures, or if a real
    /// camera under load evicted aggressively. Caller's right
    /// move is to re-capture.
    #[error("frame {0} evicted before retrieval")]
    FrameEvicted(FrameId),
    /// Detector returned an error. Distinct from "no defects" —
    /// `Ok(vec![])` is a clean pass.
    #[error("inference: {0}")]
    Inference(#[from] DetectorError),
    /// Sealing the sensitive frame failed. Caller must NOT persist
    /// the unsealed bytes — return path skips the ledger write so
    /// no plaintext leaks into the audit trail.
    #[error("sealing: {0}")]
    Sealing(#[from] SealError),
    /// The frame's `privacy` requires sealing but the pipeline was
    /// constructed without a `FrameSealer`. Operator misconfig —
    /// surface loudly so the camera-binding step gets fixed rather
    /// than silently storing plaintext.
    #[error("sensitive frame from camera {camera_id} but no sealer configured")]
    SealerMissing { camera_id: CameraId },
}

/// One audit-row's worth of inspection metadata. Mirrors the
/// columns the future `actuator_commands` Supabase table will have
/// — pure data, no behavior. The `defects` field is embedded
/// because a typical inspection produces 0..=a few defects; a
/// pathological 100-defect frame would still be a sub-kilobyte
/// JSON blob in the audit log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InspectionEvent {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    pub camera_id: CameraId,
    pub frame_id: FrameId,
    pub privacy: PrivacyClass,
    pub defects: Vec<Defect>,
    /// Length of the bytes that were (or would be) written to
    /// durable storage. After sealing this includes the AEAD
    /// nonce + tag overhead.
    pub stored_byte_len: usize,
    /// `true` if the bytes were passed through `FrameSealer::seal`
    /// before reaching `stored_byte_len`. Lets an auditor confirm
    /// "every Sensitive-class row has sealed=true" without
    /// re-deriving from the privacy field.
    pub sealed: bool,
    /// Kebab-case name of the detector that produced the verdict —
    /// from `DefectDetector::detector_name`. Logged so an auditor
    /// can answer "which detector flagged this?".
    pub detector_name: String,
}

/// What `inspect()` returns to the caller. The pipeline already
/// recorded the event to the ledger; this struct just exposes the
/// outputs for any post-processing (the V10 sync layer will pick
/// `stored_bytes` up and route it through the outbox).
#[derive(Clone, Debug)]
pub struct InspectionResult {
    pub event: InspectionEvent,
    pub frame_id: FrameId,
    pub defects: Vec<Defect>,
    /// Bytes ready for durable storage. For `Public`/`OperatorOnly`
    /// frames these are the camera's raw bytes; for `Sensitive`
    /// frames these are the sealed envelope output.
    pub stored_bytes: Vec<u8>,
}

/// Append-only in-memory ledger. Same pattern as
/// `aether_healing::HealingLedger` — production swaps the backing
/// store for a Supabase row writer (future session) without
/// changing the trait surface here.
#[derive(Default)]
pub struct InspectionLedger {
    events: Mutex<Vec<InspectionEvent>>,
}

impl InspectionLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn record(&self, event: InspectionEvent) {
        self.events.lock().await.push(event);
    }

    pub async fn snapshot(&self) -> Vec<InspectionEvent> {
        self.events.lock().await.clone()
    }

    pub async fn len(&self) -> usize {
        self.events.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

/// Orchestrator. Cheap to construct; the three trait objects are
/// `Arc`-shared so the same camera/detector/sealer can drive
/// multiple pipeline instances (e.g. one per shell-side worker).
pub struct InspectionPipeline {
    camera: Arc<dyn Camera>,
    detector: Arc<dyn DefectDetector>,
    sealer: Option<Arc<dyn FrameSealer>>,
    ledger: Arc<InspectionLedger>,
}

impl InspectionPipeline {
    pub fn new(
        camera: Arc<dyn Camera>,
        detector: Arc<dyn DefectDetector>,
        ledger: Arc<InspectionLedger>,
    ) -> Self {
        Self {
            camera,
            detector,
            sealer: None,
            ledger,
        }
    }

    /// Builder: attach a sealer. Required for cameras whose
    /// `privacy_class()` is `Sensitive`; optional otherwise.
    pub fn with_sealer(mut self, sealer: Arc<dyn FrameSealer>) -> Self {
        self.sealer = Some(sealer);
        self
    }

    pub fn ledger(&self) -> &Arc<InspectionLedger> {
        &self.ledger
    }

    /// Run one full inspection: capture → retrieve → detect →
    /// (seal if sensitive) → record → return.
    ///
    /// The `permit` is consumed by the camera dispatch — this
    /// matches the single-use semantics from V1's actuator gate.
    /// If the operator wants to inspect twice, they need two
    /// permits.
    pub async fn inspect(
        &self,
        capture_cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<InspectionResult, PipelineError> {
        // 1. Dispatch capture.
        let dispatch_result = self.camera.dispatch(capture_cmd, permit).await?;

        // 2. Extract frame_id from the result's metadata payload.
        // V2's camera contract guarantees `data.frame_id` for any
        // successful Capture; anything else is an impl bug.
        let frame_id_val = dispatch_result
            .data
            .get("frame_id")
            .ok_or_else(|| {
                PipelineError::BadDispatchResult("missing `frame_id` in dispatch result".into())
            })?
            .clone();
        let frame_id: FrameId = serde_json::from_value(frame_id_val)
            .map_err(|e| PipelineError::BadDispatchResult(format!("frame_id decode: {e}")))?;

        // 3. Retrieve frame from camera's rolling buffer. Eviction
        // before this point manifests as None.
        let mut frame = self
            .camera
            .frame_by_id(frame_id)
            .await
            .ok_or(PipelineError::FrameEvicted(frame_id))?;

        // 4. Run detector. Inference errors propagate up; an empty
        // Vec is a clean pass, NOT an error.
        let defects = self.detector.detect(&frame).await?;

        // 5. Seal if the frame's privacy class requires it. We
        // mutate `frame.bytes` so the audit row's byte-len reflects
        // the post-seal size; the original plaintext bytes only
        // ever existed in the camera's buffer, which evicts.
        let sealed = if frame.privacy.requires_encryption() {
            let sealer = self.sealer.as_ref().ok_or(PipelineError::SealerMissing {
                camera_id: self.camera.camera_id(),
            })?;
            frame.bytes = sealer.seal(&frame.bytes).await?;
            true
        } else {
            false
        };

        // 6. Build + record the audit event.
        let event = InspectionEvent {
            id: Uuid::new_v4(),
            at: Utc::now(),
            camera_id: self.camera.camera_id(),
            frame_id,
            privacy: frame.privacy,
            defects: defects.clone(),
            stored_byte_len: frame.bytes.len(),
            sealed,
            detector_name: self.detector.detector_name().to_string(),
        };
        self.ledger.record(event.clone()).await;

        Ok(InspectionResult {
            event,
            frame_id,
            defects,
            stored_bytes: frame.bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::MockCamera;
    use crate::sealer::{MockSealer, MOCK_SEAL_PREFIX};
    use crate::threshold_detector::{ThresholdDetector, CLASS_UNIFORM};
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use uuid::Uuid as TestUuid;

    fn capture_cmd() -> ActuatorCommand {
        ActuatorCommand::Capture {
            quality: 85,
            format: "raw".into(),
        }
    }

    /// Mint a permit via the production gate path. `pub(crate)`
    /// `new_unchecked` is off-limits to external code; tests must
    /// go through the real interlock.
    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: TestUuid::nil(),
            machine_id: TestUuid::nil(),
            user_certs: vec![Certification {
                user_id: TestUuid::nil(),
                code: "camera-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["camera-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock should approve in test fixture")
    }

    /// Build a pipeline with a uniform-bytes camera (will trip the
    /// threshold detector's "uniform-image" defect) and an
    /// in-memory ledger.
    fn uniform_pipeline() -> (
        Arc<MockCamera>,
        Arc<ThresholdDetector>,
        Arc<InspectionLedger>,
        InspectionPipeline,
    ) {
        let cam = Arc::new(MockCamera::new().with_bytes(vec![128; 256]));
        let det = Arc::new(ThresholdDetector::new());
        let ledger = Arc::new(InspectionLedger::new());
        let pipeline = InspectionPipeline::new(
            cam.clone() as Arc<dyn Camera>,
            det.clone() as Arc<dyn DefectDetector>,
            ledger.clone(),
        );
        (cam, det, ledger, pipeline)
    }

    #[tokio::test]
    async fn happy_path_capture_detect_record_for_public_frame() {
        let (cam, _det, ledger, pipeline) = uniform_pipeline();
        let result = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap();

        // Detector flagged the uniform frame.
        assert_eq!(result.defects.len(), 1);
        assert_eq!(result.defects[0].class, CLASS_UNIFORM);
        // Public frame → not sealed → stored_bytes are the camera's
        // original bytes.
        assert!(!result.event.sealed);
        assert_eq!(result.stored_bytes, vec![128; 256]);
        assert_eq!(result.event.stored_byte_len, 256);
        // Ledger has the event.
        assert_eq!(ledger.len().await, 1);
        let recorded = &ledger.snapshot().await[0];
        assert_eq!(recorded.camera_id, cam.camera_id());
        assert_eq!(recorded.frame_id, result.frame_id);
        assert_eq!(recorded.detector_name, "threshold");
    }

    #[tokio::test]
    async fn pass_frame_with_no_defects_still_records_event() {
        // A mid-variance scene → empty defect Vec. The pipeline
        // STILL writes the audit row — auditors need evidence of
        // every inspection, not just the failing ones.
        let cam = Arc::new(MockCamera::new().with_bytes((0u8..=255).collect()));
        let det = Arc::new(ThresholdDetector::new());
        let ledger = Arc::new(InspectionLedger::new());
        let pipeline = InspectionPipeline::new(
            cam as Arc<dyn Camera>,
            det as Arc<dyn DefectDetector>,
            ledger.clone(),
        );

        let result = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap();
        assert!(result.defects.is_empty());
        assert_eq!(ledger.len().await, 1);
        assert!(ledger.snapshot().await[0].defects.is_empty());
    }

    #[tokio::test]
    async fn sensitive_frame_with_sealer_outputs_sealed_bytes() {
        // PrivacyClass::Sensitive → pipeline MUST route through
        // sealer. MockSealer prepends MOCK_SEAL_PREFIX so the
        // test can assert opaquely.
        let cam = Arc::new(
            MockCamera::new()
                .with_privacy(PrivacyClass::Sensitive)
                .with_bytes(vec![0xAA; 32]),
        );
        let det = Arc::new(ThresholdDetector::new());
        let ledger = Arc::new(InspectionLedger::new());
        let sealer = Arc::new(MockSealer::new()) as Arc<dyn FrameSealer>;
        let pipeline = InspectionPipeline::new(
            cam as Arc<dyn Camera>,
            det as Arc<dyn DefectDetector>,
            ledger.clone(),
        )
        .with_sealer(sealer);

        let result = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap();
        assert!(result.stored_bytes.starts_with(MOCK_SEAL_PREFIX));
        assert!(result.event.sealed);
        assert_eq!(
            result.event.stored_byte_len,
            MOCK_SEAL_PREFIX.len() + 32,
            "stored_byte_len reflects POST-seal size"
        );
        // Privacy class survives onto the audit row.
        assert_eq!(result.event.privacy, PrivacyClass::Sensitive);
    }

    #[tokio::test]
    async fn sensitive_frame_without_sealer_surfaces_sealer_missing() {
        // Critical safety property: a sensitive camera bound to a
        // pipeline that lacks a sealer MUST NOT silently store
        // plaintext. Surface SealerMissing so the operator fixes
        // the binding.
        let cam = Arc::new(MockCamera::new().with_privacy(PrivacyClass::Sensitive));
        let det = Arc::new(ThresholdDetector::new());
        let ledger = Arc::new(InspectionLedger::new());
        let pipeline = InspectionPipeline::new(
            cam.clone() as Arc<dyn Camera>,
            det as Arc<dyn DefectDetector>,
            ledger.clone(),
        );
        // No .with_sealer() — deliberate.
        let err = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, PipelineError::SealerMissing { .. }));
        // And no ledger event was written — a failed inspection
        // doesn't pollute the audit trail with half-state.
        assert_eq!(ledger.len().await, 0);
    }

    #[tokio::test]
    async fn sealer_failure_propagates_and_does_not_record_event() {
        // MockSealer::always_fail simulates a cryptographic failure
        // (key rotated mid-session, HSM unreachable, etc.). The
        // pipeline must NOT write an audit row for a failed
        // inspection — half-records would mislead an auditor.
        let cam = Arc::new(MockCamera::new().with_privacy(PrivacyClass::Sensitive));
        let det = Arc::new(ThresholdDetector::new());
        let ledger = Arc::new(InspectionLedger::new());
        let sealer = Arc::new(MockSealer::new().always_fail()) as Arc<dyn FrameSealer>;
        let pipeline = InspectionPipeline::new(
            cam as Arc<dyn Camera>,
            det as Arc<dyn DefectDetector>,
            ledger.clone(),
        )
        .with_sealer(sealer);

        let err = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, PipelineError::Sealing(_)));
        assert_eq!(ledger.len().await, 0);
    }

    #[tokio::test]
    async fn permit_consumed_before_dispatch_surfaces_as_dispatch_error() {
        // Permit lifecycle is V1's responsibility; the pipeline
        // just propagates. We exercise the path here so a future
        // refactor that swallows the error gets caught.
        let (_cam, _det, ledger, pipeline) = uniform_pipeline();
        // Expired permit via short TTL.
        let req = UnlockRequest {
            user_id: TestUuid::nil(),
            machine_id: TestUuid::nil(),
            user_certs: vec![Certification {
                user_id: TestUuid::nil(),
                code: "camera-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["camera-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        let permit =
            aether_actuators::gate::gate_with_ttl(&req, aether_actuators::permit::PermitTtl(20))
                .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;

        let err = pipeline.inspect(capture_cmd(), permit).await.unwrap_err();
        assert!(matches!(err, PipelineError::Dispatch(_)));
        // No ledger pollution.
        assert_eq!(ledger.len().await, 0);
    }

    #[tokio::test]
    async fn ledger_accumulates_across_multiple_inspections() {
        // Multi-inspection sanity — three captures, three audit rows
        // in append order. Each call gets a fresh permit (single-
        // use semantics from V1).
        let (_cam, _det, ledger, pipeline) = uniform_pipeline();
        for _ in 0..3 {
            pipeline
                .inspect(capture_cmd(), test_permit())
                .await
                .unwrap();
        }
        assert_eq!(ledger.len().await, 3);
    }

    #[tokio::test]
    async fn inspection_event_serde_round_trips() {
        // The audit row will eventually JSON-serialize to a Supabase
        // column. Pin the shape so a future schema migration knows
        // the canonical layout.
        let (_cam, _det, _ledger, pipeline) = uniform_pipeline();
        let result = pipeline
            .inspect(capture_cmd(), test_permit())
            .await
            .unwrap();
        let s = serde_json::to_string(&result.event).unwrap();
        let back: InspectionEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, result.event.id);
        assert_eq!(back.camera_id, result.event.camera_id);
        assert_eq!(back.frame_id, result.event.frame_id);
        assert_eq!(back.privacy, result.event.privacy);
        assert_eq!(back.defects.len(), result.event.defects.len());
        assert_eq!(back.sealed, result.event.sealed);
        assert_eq!(back.detector_name, result.event.detector_name);
    }

    #[tokio::test]
    async fn ledger_is_empty_helper_distinguishes_zero_from_some() {
        let ledger = InspectionLedger::new();
        assert!(ledger.is_empty().await);
        assert_eq!(ledger.len().await, 0);
    }
}
