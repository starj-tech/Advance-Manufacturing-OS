//! Machine-vision integration — cameras as bidirectional [`Actuator`]s.
//!
//! ## Where this sits
//! Built on top of `aether-actuators`: every camera is an `Actuator`
//! that responds to `ActuatorCommand::Capture` by emitting a [`Frame`].
//! The Actuator permit/interlock gate from V1 carries over — no
//! camera frame is captured without the safety check first passing.
//!
//! ## Why frames travel out-of-band
//! The Actuator trait returns `ActuatorResult` whose `data` is a
//! `serde_json::Value`. Stuffing image bytes inside JSON would
//! base64-inflate the payload by ~33% and bloat the
//! `actuator_commands` audit row. Instead:
//!
//!   * `dispatch(Capture {...})` returns `data = { "frame_id": "...",
//!     "width": ..., "height": ..., "privacy": ... }` — small,
//!     audit-friendly metadata only.
//!   * The full [`Frame`] (with bytes) is buffered inside the camera
//!     and retrieved via [`Camera::frame_by_id`] — the
//!     `InspectionPipeline` (Session V4) does this immediately after
//!     a successful capture.
//!
//! That separation also lets the pipeline decide encryption: a frame
//! with [`PrivacyClass::Sensitive`] gets sealed via
//! `aether-crypto::envelope` BEFORE it touches the outbox or any
//! durable storage. Public frames travel as-is.
//!
//! ## What's NOT here yet
//! Defect detection (`DefectDetector` trait + ML inference) ships in
//! Session V3. Capture-then-detect orchestration ships in Session
//! V4. This file owns the camera-side surface: trait, frame, privacy.

pub mod camera;
pub mod frame;
pub mod privacy;

pub use camera::{Camera, CameraCapabilities, MockCamera};
pub use frame::{Frame, FrameId, PixelFormat};
pub use privacy::PrivacyClass;
