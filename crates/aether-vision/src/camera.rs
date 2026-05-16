//! [`Camera`] — supertrait over [`aether_actuators::Actuator`] that
//! adds vision-specific accessors.
//!
//! ## Why a supertrait
//! Every camera IS an actuator (it accepts `Capture` and `Halt`),
//! plus a camera has vision-specific state the pipeline needs:
//! capabilities, privacy classification, the buffer of recently-
//! captured frames. Modelling those as a supertrait means a `&dyn
//! Camera` lets the caller use both surfaces without casting.
//!
//! ## Why frames are buffered, not returned inline
//! See `lib.rs` — `ActuatorResult.data` is `serde_json::Value`, too
//! narrow for image bytes. Capture writes the frame into an
//! internal buffer and returns the frame id in `data`. The
//! pipeline calls [`Camera::frame_by_id`] right after the
//! successful dispatch to retrieve the actual bytes.
//!
//! ## Why `frame_by_id` returns `Option<Frame>`, not `Result`
//! Eviction is the expected steady state — cameras keep a small
//! rolling buffer (default `MOCK_BUFFER_CAPACITY` for the mock).
//! `None` simply means "you waited too long". A `Result` would
//! force every caller into a useless match arm; `Option` reads
//! better at the call site (`.ok_or(MyError::FrameEvicted)?`).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, CameraId};

use crate::frame::{Frame, FrameId, PixelFormat};
use crate::privacy::PrivacyClass;

/// What a camera advertises to the pipeline. Pinned at construction
/// (real cameras might re-query the device on connect; the mock
/// hard-codes its own values).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraCapabilities {
    /// Pixel formats the camera can emit. The first entry is the
    /// "preferred" / native format; the pipeline asks for that
    /// unless explicitly overridden.
    pub formats: Vec<PixelFormat>,
    pub max_width: u32,
    pub max_height: u32,
    /// Max framerate the camera advertises. Capture commands today
    /// are single-shot; this field is informational for the
    /// future burst-capture path.
    pub max_fps: u8,
}

#[async_trait]
pub trait Camera: Actuator {
    /// Stable identity. The `Actuator::actuator_id` is derived from
    /// this same UUID so audit rows linkable both ways.
    fn camera_id(&self) -> CameraId;

    /// Static description of the camera's abilities.
    fn capabilities(&self) -> &CameraCapabilities;

    /// Privacy classification this camera produces. Inherited onto
    /// every emitted [`Frame`]; the pipeline reads this to decide
    /// "seal or not" before storage.
    fn privacy_class(&self) -> PrivacyClass;

    /// Look up a previously captured frame by id. Returns `None` if
    /// the frame has been evicted from the rolling buffer (or never
    /// existed). Callers translate to their own error variant if
    /// they need it as a hard failure.
    async fn frame_by_id(&self, id: FrameId) -> Option<Frame>;
}

// ---------- MockCamera ----------

/// Rolling-buffer capacity for the mock camera. Small on purpose —
/// real cameras evict aggressively because frames are large; the
/// pipeline is expected to fetch within a tight window.
pub const MOCK_BUFFER_CAPACITY: usize = 8;

/// In-process camera mock. Returns canned bytes for every Capture;
/// records every received command for test inspection. Default
/// capabilities: 1 format (PNG), 640x480, 30 fps, [`PrivacyClass::
/// Public`]. All overrideable via builder methods.
///
/// ## Why `received` is a separate buffer from `frames`
/// `received` records EVERY command (including `Halt` and bad
/// commands) for assertion-style testing. `frames` stores ONLY
/// successfully-captured images, capped at `MOCK_BUFFER_CAPACITY`.
/// Tests that check command flow look at `received`; tests that
/// check frame retrieval look at `frames`.
pub struct MockCamera {
    camera_id: CameraId,
    capabilities: CameraCapabilities,
    privacy: PrivacyClass,
    canned_bytes: Vec<u8>,
    canned_width: u32,
    canned_height: u32,
    canned_format: PixelFormat,
    received: Mutex<Vec<ActuatorCommand>>,
    frames: Mutex<Vec<Frame>>,
}

impl MockCamera {
    pub fn new() -> Self {
        Self {
            camera_id: CameraId::new(),
            capabilities: CameraCapabilities {
                formats: vec![PixelFormat::Png],
                max_width: 640,
                max_height: 480,
                max_fps: 30,
            },
            privacy: PrivacyClass::Public,
            // 8-byte canned blob. Real cameras emit kilobytes-to-
            // megabytes; this is a deliberate marker for tests so
            // the bytes are easy to compare verbatim.
            canned_bytes: vec![0xCA, 0xFE, 0xBA, 0xBE, 0xDE, 0xAD, 0xBE, 0xEF],
            canned_width: 640,
            canned_height: 480,
            canned_format: PixelFormat::Png,
            received: Mutex::new(Vec::new()),
            frames: Mutex::new(Vec::new()),
        }
    }

    /// Builder: override the privacy class. Tests for the sensitive-
    /// frame path use this to flip the `requires_encryption()` flag
    /// without standing up a separate camera type.
    pub fn with_privacy(mut self, privacy: PrivacyClass) -> Self {
        self.privacy = privacy;
        self
    }

    /// Builder: replace the canned bytes a Capture returns. Use a
    /// distinct marker per test so multi-frame assertions can tell
    /// frames apart.
    pub fn with_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.canned_bytes = bytes;
        self
    }

    /// Builder: change the reported resolution. Production cameras
    /// vary widely (640x480 spot-check, 4K inspection); the mock
    /// lets tests pick a shape that exercises the metadata path.
    pub fn with_resolution(mut self, w: u32, h: u32) -> Self {
        self.canned_width = w;
        self.canned_height = h;
        self.capabilities.max_width = w.max(self.capabilities.max_width);
        self.capabilities.max_height = h.max(self.capabilities.max_height);
        self
    }

    /// Snapshot every command the camera has seen so far. Used by
    /// tests to assert pipeline-driven command sequences.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }

    /// Snapshot every successfully captured frame. Used by tests
    /// that don't go through `frame_by_id` (e.g. asserting count).
    pub async fn captured_frames(&self) -> Vec<Frame> {
        self.frames.lock().await.clone()
    }

    /// Internal: push a fresh frame onto the rolling buffer,
    /// evicting the oldest if at capacity. Pure synchronous on the
    /// already-held lock.
    fn push_frame(frames: &mut Vec<Frame>, frame: Frame) {
        if frames.len() >= MOCK_BUFFER_CAPACITY {
            frames.remove(0);
        }
        frames.push(frame);
    }
}

impl Default for MockCamera {
    fn default() -> Self {
        Self::new()
    }
}

/// Cheap-clone handle for sharing the mock across spawned tasks.
pub type MockCameraRef = Arc<MockCamera>;

#[async_trait]
impl Actuator for MockCamera {
    fn actuator_id(&self) -> ActuatorId {
        // Camera id and Actuator id share the underlying UUID so the
        // audit row links both ways without a separate join table.
        ActuatorId::from_uuid(self.camera_id.into_uuid())
    }

    fn actuator_kind(&self) -> &'static str {
        "camera"
    }

    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        // Record EVERY command, including the ones we reject. Lets
        // tests assert "the pipeline tried to dispatch X" even when
        // the camera said no.
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::Capture { quality, format } => {
                let frame = Frame {
                    id: FrameId::new(),
                    camera_id: self.camera_id,
                    captured_at: Utc::now(),
                    width: self.canned_width,
                    height: self.canned_height,
                    format: self.canned_format.clone(),
                    privacy: self.privacy,
                    bytes: self.canned_bytes.clone(),
                };
                let frame_id = frame.id;
                {
                    let mut buf = self.frames.lock().await;
                    Self::push_frame(&mut buf, frame);
                }
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "capture".into(),
                    completed_at: Utc::now(),
                    // Metadata-only — image bytes travel via
                    // `frame_by_id`. Keeps the audit row small.
                    data: serde_json::json!({
                        "frame_id": frame_id,
                        "width": self.canned_width,
                        "height": self.canned_height,
                        "privacy": self.privacy.slug(),
                        "format": self.canned_format.slug(),
                        "requested_quality": quality,
                        "requested_format": format,
                    }),
                })
            }
            ActuatorCommand::Halt => Ok(ActuatorResult {
                actuator_id: self.actuator_id(),
                command_kind: "halt".into(),
                completed_at: Utc::now(),
                data: serde_json::json!({"halted": true}),
            }),
            other => Err(ActuatorError::BadCommand(format!(
                "camera does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl Camera for MockCamera {
    fn camera_id(&self) -> CameraId {
        self.camera_id
    }

    fn capabilities(&self) -> &CameraCapabilities {
        &self.capabilities
    }

    fn privacy_class(&self) -> PrivacyClass {
        self.privacy
    }

    async fn frame_by_id(&self, id: FrameId) -> Option<Frame> {
        self.frames
            .lock()
            .await
            .iter()
            .find(|f| f.id == id)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_actuators::gate::gate;
    use aether_actuators::permit::{ActuatorPermit, PermitTtl};
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use uuid::Uuid;

    fn capture_cmd() -> ActuatorCommand {
        ActuatorCommand::Capture {
            quality: 85,
            format: "png".into(),
        }
    }

    /// Build a permit through the production `gate()` path. The
    /// permit's `new_unchecked` constructor is `pub(crate)` to
    /// `aether-actuators` — external code (including vision tests)
    /// MUST mint permits via the interlock check. We construct a
    /// minimal `UnlockRequest` that the interlock approves.
    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
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

    /// Custom-TTL permit for the expiry-path tests.
    fn test_permit_with_ttl(ttl_ms: i64) -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
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
        aether_actuators::gate::gate_with_ttl(&req, PermitTtl(ttl_ms))
            .expect("interlock should approve in test fixture")
    }

    #[tokio::test]
    async fn capture_emits_frame_with_camera_metadata_inherited() {
        let cam = MockCamera::new().with_privacy(PrivacyClass::OperatorOnly);
        let result = cam.dispatch(capture_cmd(), test_permit()).await.unwrap();

        // Audit payload carries only metadata, no bytes.
        let frame_id_str = result.data["frame_id"].as_str().unwrap();
        assert!(!frame_id_str.is_empty());
        assert_eq!(result.data["privacy"], "operator-only");
        assert_eq!(result.data["width"], 640);

        // Frame is in the buffer with the privacy class inherited
        // from the camera (load-bearing — see frame.rs round-trip
        // test).
        let frames = cam.captured_frames().await;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].privacy, PrivacyClass::OperatorOnly);
        assert_eq!(frames[0].camera_id, cam.camera_id());
    }

    #[tokio::test]
    async fn frame_by_id_retrieves_the_just_captured_frame() {
        let cam = MockCamera::new().with_bytes(vec![0xAA, 0xBB]);
        let result = cam.dispatch(capture_cmd(), test_permit()).await.unwrap();

        // Parse the frame_id back out of the audit JSON — the same
        // path the pipeline (V4) will use.
        let frame_id: FrameId = serde_json::from_value(result.data["frame_id"].clone()).unwrap();
        let frame = cam.frame_by_id(frame_id).await.expect("found in buffer");
        assert_eq!(frame.bytes, vec![0xAA, 0xBB]);
    }

    #[tokio::test]
    async fn frame_by_id_returns_none_for_unknown_id() {
        // No frames captured yet — any id is unknown.
        let cam = MockCamera::new();
        assert!(cam.frame_by_id(FrameId::new()).await.is_none());
    }

    #[tokio::test]
    async fn rolling_buffer_evicts_oldest_at_capacity() {
        let cam = MockCamera::new();
        let mut earliest_id: Option<FrameId> = None;

        // Push CAPACITY+1 captures; the first one should be evicted.
        for i in 0..=MOCK_BUFFER_CAPACITY {
            let result = cam.dispatch(capture_cmd(), test_permit()).await.unwrap();
            if i == 0 {
                earliest_id =
                    Some(serde_json::from_value(result.data["frame_id"].clone()).unwrap());
            }
        }

        let frames = cam.captured_frames().await;
        assert_eq!(frames.len(), MOCK_BUFFER_CAPACITY);
        // Earliest frame evicted — buffer is rolling.
        assert!(cam.frame_by_id(earliest_id.unwrap()).await.is_none());
    }

    #[tokio::test]
    async fn move_joint_rejected_by_camera_with_bad_command() {
        // A camera receiving a robot command is a routing bug;
        // surface it loudly, don't silently ignore.
        let cam = MockCamera::new();
        let err = cam
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
    async fn halt_is_accepted_universally_per_plan_contract() {
        // Every Actuator MUST honor `Halt` — the universal preemption
        // command. A camera that rejected halt would break the
        // emergency-stop story (V8).
        let cam = MockCamera::new();
        let result = cam
            .dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        assert_eq!(result.data["halted"], true);
    }

    #[tokio::test]
    async fn capabilities_advertise_at_least_one_format() {
        // The pipeline asks for `capabilities().formats[0]` as the
        // preferred format; an empty list would panic the caller.
        let cam = MockCamera::new();
        assert!(!cam.capabilities().formats.is_empty());
    }

    #[tokio::test]
    async fn camera_id_and_actuator_id_share_underlying_uuid() {
        // Linkability property: the audit row's actuator_id and the
        // vision_frames row's camera_id are the same UUID, just
        // newtype-wrapped differently. A test confirms the
        // round-trip works.
        let cam = MockCamera::new();
        assert_eq!(cam.camera_id().into_uuid(), cam.actuator_id().into_uuid());
    }

    #[tokio::test]
    async fn dispatch_with_expired_permit_rejected_before_recording() {
        // The PermitConsumed path requires `pub(crate)` access we
        // don't have from outside aether-actuators. We exercise the
        // sibling failure (PermitExpired) instead — same property,
        // namely that enforce_permit runs FIRST so a stale permit
        // doesn't pollute the received-commands log.
        let cam = MockCamera::new();
        let permit = test_permit_with_ttl(20);
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        let err = cam.dispatch(capture_cmd(), permit).await.unwrap_err();
        assert!(matches!(err, ActuatorError::PermitExpired(_)));
        // Critical: the received buffer stays empty.
        assert!(cam.received().await.is_empty());
    }

    #[tokio::test]
    async fn full_chain_gate_to_frame_retrieval() {
        // End-to-end happy path through the public API:
        // gate(UnlockRequest) → permit → dispatch(Capture) →
        // frame_id → frame_by_id → bytes. Pinned for a Sensitive
        // camera so the privacy assertion is the meaningful one.
        let cam = MockCamera::new()
            .with_privacy(PrivacyClass::Sensitive)
            .with_bytes(vec![0x11, 0x22, 0x33, 0x44]);
        let result = cam.dispatch(capture_cmd(), test_permit()).await.unwrap();

        let frame_id: FrameId = serde_json::from_value(result.data["frame_id"].clone()).unwrap();
        let frame = cam.frame_by_id(frame_id).await.unwrap();
        assert_eq!(frame.bytes, vec![0x11, 0x22, 0x33, 0x44]);
        // Sensitive frame — pipeline must seal these before storage.
        assert!(frame.privacy.requires_encryption());
    }
}
