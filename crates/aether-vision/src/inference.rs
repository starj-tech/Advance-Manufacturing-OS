//! `DefectDetector` — the abstraction over machine-vision inference.
//!
//! ## Why a trait
//! Production quality inspection uses one of several wildly different
//! backends:
//!
//!   * Threshold / classical CV (Sobel, blob detection) — cheap,
//!     deterministic, easy to audit. Useful as a first-pass filter
//!     and as a fallback when ML weights are unavailable.
//!   * ONNX Runtime executing a CNN trained off-tenant — typical for
//!     surface-defect / missing-component checks.
//!   * Vendor-specific SDK (Cognex, Keyence) over a TCP / HTTP socket.
//!
//! All three answer the same question: "given this frame, list the
//! defects". The trait pins that contract so the V4 pipeline can
//! consume any of them interchangeably.
//!
//! ## What lands in this session vs. future
//! V3 ships:
//!   * `DefectDetector` trait + `Defect`, `BoundingBox`,
//!     `DetectorError`.
//!   * `ThresholdDetector` — pure-Rust variance heuristic. Genuinely
//!     useful as a sanity check (stuck-image bug → zero variance;
//!     sensor noise → high variance) AND testable without model
//!     weights.
//!
//! Future sessions wire `OnnxDetector` behind an `inference` Cargo
//! feature. The trait surface stays unchanged — only the impl slots
//! in.
//!
//! ## Confidence convention
//! 0.0..=1.0; higher means "more sure it's a defect". Callers (the
//! V4 pipeline) typically gate "reject the part" on confidence ≥ a
//! per-tenant threshold (default 0.85). The trait doesn't enforce
//! that — implementations are encouraged to be conservative (a
//! deterministic ThresholdDetector returns 1.0 only for flagrant
//! violations; ML detectors should calibrate against a holdout).

use crate::frame::Frame;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DetectorError {
    /// The frame's pixel format isn't one this detector can decode.
    /// Surfaces explicitly rather than silently passing — a mismatch
    /// usually indicates a camera-detector misconfiguration in the
    /// V4 pipeline, which should be flagged not hidden.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    /// The frame structurally couldn't be processed — empty bytes,
    /// dimensions that don't match the byte count, etc.
    #[error("bad frame: {0}")]
    BadFrame(String),
    /// Inference runtime failure. For deterministic detectors this
    /// shouldn't fire; ONNX detectors map runtime errors here.
    #[error("inference: {0}")]
    Inference(String),
}

/// Pixel-coordinate bounding box. Top-left origin (standard for
/// computer-vision libraries). All fields in pixels; dimensions are
/// `u32` so a 4K frame is representable.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl BoundingBox {
    /// Bounding box covering the entire frame. Used by detectors
    /// that flag a defect without localizing it (e.g. global
    /// brightness/variance checks).
    pub fn full_frame(frame: &Frame) -> Self {
        Self {
            x: 0,
            y: 0,
            w: frame.width,
            h: frame.height,
        }
    }

    /// `true` if this box overlaps `other` (zero-area touch counts
    /// as overlap). Used by the pipeline's non-max suppression in
    /// future — bundled here so multiple detectors can chain.
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        let self_right = self.x + self.w;
        let self_bottom = self.y + self.h;
        let other_right = other.x + other.w;
        let other_bottom = other.y + other.h;
        self.x < other_right
            && other.x < self_right
            && self.y < other_bottom
            && other.y < self_bottom
    }

    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }
}

/// One detected defect. `class` is a free-form kebab-case identifier
/// owned by the detector — `ThresholdDetector` emits `"uniform-image"`
/// and `"high-variance"`; an ONNX detector would emit the class names
/// from its training labels (`"scratch"`, `"missing-component"`,
/// etc.). The pipeline does not introspect `class`; it surfaces it
/// in the audit row and lets a tenant-side rule decide severity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Defect {
    pub class: String,
    pub bbox: BoundingBox,
    /// 0.0..=1.0 — higher means "more sure". See module docs.
    pub confidence: f32,
}

#[async_trait]
pub trait DefectDetector: Send + Sync {
    /// Run inference. Returns zero or more defects. Empty Vec
    /// means "frame passed inspection"; non-empty means at least
    /// one defect was identified.
    async fn detect(&self, frame: &Frame) -> Result<Vec<Defect>, DetectorError>;

    /// Short kebab-case identifier (`"threshold"`, `"onnx-yolov8"`).
    /// Logged into the audit row so an auditor can see which
    /// detector decided.
    fn detector_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_full_frame_matches_dimensions() {
        use crate::{FrameId, PixelFormat, PrivacyClass};
        use aether_core::CameraId;
        use chrono::Utc;

        let frame = Frame {
            id: FrameId::new(),
            camera_id: CameraId::new(),
            captured_at: Utc::now(),
            width: 640,
            height: 480,
            format: PixelFormat::Png,
            privacy: PrivacyClass::Public,
            bytes: vec![],
        };
        let bbox = BoundingBox::full_frame(&frame);
        assert_eq!(bbox.x, 0);
        assert_eq!(bbox.y, 0);
        assert_eq!(bbox.w, 640);
        assert_eq!(bbox.h, 480);
        assert_eq!(bbox.area(), 640 * 480);
    }

    #[test]
    fn bbox_intersects_overlapping_boxes() {
        let a = BoundingBox {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let b = BoundingBox {
            x: 50,
            y: 50,
            w: 100,
            h: 100,
        };
        assert!(a.intersects(&b));
        assert!(b.intersects(&a)); // symmetry
    }

    #[test]
    fn bbox_does_not_intersect_disjoint_boxes() {
        let a = BoundingBox {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        let b = BoundingBox {
            x: 100,
            y: 100,
            w: 10,
            h: 10,
        };
        assert!(!a.intersects(&b));
    }

    #[test]
    fn bbox_touching_edge_is_treated_as_non_overlap() {
        // The pipeline's non-max suppression treats adjacent-but-not-
        // overlapping defects as distinct. A defect at x=0..=99 and
        // one at x=100..=199 share an edge but no pixels — NMS
        // should keep both.
        let a = BoundingBox {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        };
        let b = BoundingBox {
            x: 100,
            y: 0,
            w: 100,
            h: 100,
        };
        assert!(!a.intersects(&b));
    }

    #[test]
    fn bbox_serde_round_trip() {
        let b = BoundingBox {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        };
        let s = serde_json::to_string(&b).unwrap();
        let back: BoundingBox = serde_json::from_str(&s).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn defect_serde_round_trip_preserves_confidence_precision() {
        let d = Defect {
            class: "scratch".into(),
            bbox: BoundingBox {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            },
            confidence: 0.875_f32,
        };
        let s = serde_json::to_string(&d).unwrap();
        let back: Defect = serde_json::from_str(&s).unwrap();
        assert_eq!(back.class, d.class);
        assert_eq!(back.bbox, d.bbox);
        // f32 → JSON number → f32 should round-trip cleanly for
        // power-of-two fractions like 0.875.
        assert!((back.confidence - d.confidence).abs() < 1e-6);
    }
}
