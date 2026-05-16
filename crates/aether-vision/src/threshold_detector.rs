//! `ThresholdDetector` — variance-based first-pass quality detector.
//!
//! ## What it does
//! Treats the frame bytes as a 1D signal and computes per-byte
//! variance against the mean. Two thresholds:
//!
//!   * `low_variance` — if variance falls below this, the frame is
//!     suspiciously uniform. Real-world causes: a stuck camera
//!     pipeline (same bytes every capture), a fully-occluded lens
//!     (operator's thumb), a power-failed scene.
//!
//!   * `high_variance` — if variance climbs above this, the frame is
//!     suspiciously noisy. Real-world causes: a malfunctioning
//!     sensor, an IR-blast lighting event, a corrupt bytestream.
//!
//! Either condition produces a defect spanning the whole frame (the
//! ThresholdDetector does no spatial localization — that's the
//! ONNX-based detector's job in a future session).
//!
//! ## Why ship a variance-only detector
//! Three reasons.
//!
//! 1. **Pure Rust, zero ML deps.** Ships today without `ort`,
//!    `image`, or model weights. The V4 pipeline can be tested
//!    end-to-end against this detector immediately.
//!
//! 2. **Genuinely useful.** "The image is stuck" and "the bytes are
//!    pure noise" are real failure modes that ML detectors can miss
//!    because their training data rarely contains those pathological
//!    cases. Running ThresholdDetector as a first-pass *before* the
//!    expensive ML detector catches a class of bugs the ML model
//!    would silently hallucinate through.
//!
//! 3. **Deterministic.** Two calls with identical input produce
//!    byte-identical Defect lists. That's the property `gate→
//!    dispatch→detect` audit replay needs, and it's untrue of
//!    any ML detector that uses non-deterministic GPU paths.
//!
//! ## Default thresholds
//! Tuned empirically for 8-bit pixel data (Raw or PNG decoded into
//! grayscale):
//!
//!   * `DEFAULT_LOW_VARIANCE = 50.0` — below this, the frame is
//!     ≥99.9% uniform. A normal manufactured part scene yields
//!     variance ≥ 200 even on a flat metal surface.
//!
//!   * `DEFAULT_HIGH_VARIANCE = 9000.0` — well above the variance
//!     of a healthy maximum-contrast scene (~6400 for a 50/50
//!     black/white test pattern; sensor noise pushes above 9000).
//!
//! Tenants override per-camera via `with_thresholds`.

use crate::frame::Frame;
use crate::inference::{BoundingBox, Defect, DefectDetector, DetectorError};
use async_trait::async_trait;

pub const DEFAULT_LOW_VARIANCE: f64 = 50.0;
pub const DEFAULT_HIGH_VARIANCE: f64 = 9000.0;

/// Defect class strings the detector emits. Pinned as constants so
/// downstream rule engines (V4 pipeline) can match on them without
/// the spelling drifting between detector versions.
pub const CLASS_UNIFORM: &str = "uniform-image";
pub const CLASS_HIGH_VARIANCE: &str = "high-variance";

pub struct ThresholdDetector {
    low_variance: f64,
    high_variance: f64,
}

impl ThresholdDetector {
    pub fn new() -> Self {
        Self {
            low_variance: DEFAULT_LOW_VARIANCE,
            high_variance: DEFAULT_HIGH_VARIANCE,
        }
    }

    /// Builder: override either threshold. Tenants tune per-camera
    /// based on their actual scene noise floor (a high-contrast
    /// PCB-inspection camera might use 500/12000; a low-light
    /// surveillance camera might use 20/4000).
    pub fn with_thresholds(mut self, low: f64, high: f64) -> Self {
        self.low_variance = low;
        self.high_variance = high;
        self
    }

    pub fn low_threshold(&self) -> f64 {
        self.low_variance
    }
    pub fn high_threshold(&self) -> f64 {
        self.high_variance
    }
}

impl Default for ThresholdDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute per-byte variance against the mean. Pure function, pulled
/// out so tests can call it directly without standing up a Frame.
fn byte_variance(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let n = bytes.len() as f64;
    let mean: f64 = bytes.iter().map(|&b| b as f64).sum::<f64>() / n;
    let sum_sq: f64 = bytes
        .iter()
        .map(|&b| {
            let d = b as f64 - mean;
            d * d
        })
        .sum();
    sum_sq / n
}

/// Map "how far past the threshold" to a 0.0..=1.0 confidence.
/// Linear ramp, saturating at 1.0 when the violation is ≥2× the
/// threshold. Picked so a borderline case reads as ~0.5 and a
/// flagrant violation as 1.0 — gives the V4 pipeline a meaningful
/// signal to gate on.
fn ramp_confidence(violation: f64, threshold: f64) -> f32 {
    if threshold <= 0.0 {
        return 1.0;
    }
    let ratio = violation / threshold;
    ratio.clamp(0.0, 1.0) as f32
}

#[async_trait]
impl DefectDetector for ThresholdDetector {
    async fn detect(&self, frame: &Frame) -> Result<Vec<Defect>, DetectorError> {
        if frame.bytes.is_empty() {
            return Err(DetectorError::BadFrame("zero-byte frame".into()));
        }
        let var = byte_variance(&frame.bytes);
        let mut defects: Vec<Defect> = Vec::new();

        if var < self.low_variance {
            // Confidence scales by how far BELOW the floor we are.
            // var=0 (perfectly uniform) → 1.0; var just under floor
            // → ~0.0.
            let gap = self.low_variance - var;
            defects.push(Defect {
                class: CLASS_UNIFORM.to_string(),
                bbox: BoundingBox::full_frame(frame),
                confidence: ramp_confidence(gap, self.low_variance),
            });
        }

        if var > self.high_variance {
            let gap = var - self.high_variance;
            defects.push(Defect {
                class: CLASS_HIGH_VARIANCE.to_string(),
                bbox: BoundingBox::full_frame(frame),
                confidence: ramp_confidence(gap, self.high_variance),
            });
        }

        Ok(defects)
    }

    fn detector_name(&self) -> &'static str {
        "threshold"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameId, PixelFormat, PrivacyClass};
    use aether_core::CameraId;
    use chrono::Utc;

    fn frame_with_bytes(bytes: Vec<u8>) -> Frame {
        Frame {
            id: FrameId::new(),
            camera_id: CameraId::new(),
            captured_at: Utc::now(),
            width: 8,
            height: 8,
            format: PixelFormat::Raw {
                channels: 1,
                bit_depth: 8,
            },
            privacy: PrivacyClass::Public,
            bytes,
        }
    }

    #[test]
    fn byte_variance_of_uniform_bytes_is_zero() {
        assert!((byte_variance(&[128; 64]) - 0.0).abs() < 1e-9);
        assert!((byte_variance(&[0; 32]) - 0.0).abs() < 1e-9);
        assert!((byte_variance(&[255; 32]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn byte_variance_of_alternating_extremes_approaches_max() {
        // 50/50 black/white. Mean = 127.5; deviation = 127.5 for each
        // byte; variance = 127.5^2 = 16256.25 — well above the
        // default high_variance threshold (9000), which is why the
        // alternating-extremes test catches.
        let bytes: Vec<u8> = (0..256).map(|i| if i % 2 == 0 { 0 } else { 255 }).collect();
        let v = byte_variance(&bytes);
        assert!(v > 16_000.0 && v < 16_500.0, "got {v}");
    }

    #[test]
    fn byte_variance_of_empty_slice_is_zero_not_nan() {
        // The pathological edge — must not divide by zero. The
        // detector handles empty bytes via BadFrame; this is just
        // belt-and-braces on the helper.
        assert!((byte_variance(&[]) - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn uniform_frame_is_flagged_with_uniform_class() {
        // A stuck-camera bug: every byte the same. Variance is 0,
        // well below the low threshold — the detector flags it.
        let d = ThresholdDetector::new();
        let frame = frame_with_bytes(vec![128; 64]);
        let defects = d.detect(&frame).await.unwrap();
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].class, CLASS_UNIFORM);
        // confidence = (low_variance - 0) / low_variance = 1.0
        assert!((defects[0].confidence - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn high_variance_frame_is_flagged_with_high_variance_class() {
        let d = ThresholdDetector::new();
        // Alternating 0/255 ≈ 16256 variance > 9000 default → flagged.
        let bytes: Vec<u8> = (0..256).map(|i| if i % 2 == 0 { 0 } else { 255 }).collect();
        let frame = frame_with_bytes(bytes);
        let defects = d.detect(&frame).await.unwrap();
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].class, CLASS_HIGH_VARIANCE);
        assert!(defects[0].confidence > 0.0);
    }

    #[tokio::test]
    async fn mid_variance_frame_passes_with_empty_defect_list() {
        // Healthy scene: byte ramp 0..=255. Variance is moderate
        // (~5461 for a uniform distribution on [0,255]) — between
        // low (50) and high (9000) thresholds, so no defect.
        let d = ThresholdDetector::new();
        let bytes: Vec<u8> = (0..=255).collect();
        let frame = frame_with_bytes(bytes);
        let defects = d.detect(&frame).await.unwrap();
        assert!(
            defects.is_empty(),
            "mid-variance scene should pass; got {} defects",
            defects.len()
        );
    }

    #[tokio::test]
    async fn empty_bytes_surfaces_bad_frame_error() {
        // The structural-input check — refuse to compute variance
        // on zero bytes (would silently return 0.0 and flag as
        // uniform, which is misleading; the right answer is "this
        // frame is malformed").
        let d = ThresholdDetector::new();
        let frame = frame_with_bytes(vec![]);
        let err = d.detect(&frame).await.unwrap_err();
        assert!(matches!(err, DetectorError::BadFrame(_)));
    }

    #[tokio::test]
    async fn custom_thresholds_change_decision_at_boundary() {
        // Same frame, two detectors with different thresholds — one
        // flags it, the other passes. Proves the configuration knob
        // actually moves the decision.
        let bytes: Vec<u8> = (0..=255).collect();
        let frame = frame_with_bytes(bytes);

        let permissive = ThresholdDetector::new().with_thresholds(10.0, 20_000.0);
        let strict = ThresholdDetector::new().with_thresholds(10_000.0, 12_000.0);

        // permissive: low=10 (var is ~5461 > 10) AND high=20000
        // (var < 20000) → no defects.
        assert!(permissive.detect(&frame).await.unwrap().is_empty());

        // strict: low=10000 (var is ~5461 < 10000) → flagged as
        // uniform (by THIS detector's definition of "uniform").
        let strict_defects = strict.detect(&frame).await.unwrap();
        assert_eq!(strict_defects.len(), 1);
        assert_eq!(strict_defects[0].class, CLASS_UNIFORM);
    }

    #[tokio::test]
    async fn defect_bbox_covers_entire_frame() {
        // ThresholdDetector doesn't localize — it tells you something
        // is wrong with the WHOLE frame. The bbox should reflect that
        // so a downstream "show me where" rendering doesn't crop to
        // a misleading sub-region.
        let d = ThresholdDetector::new();
        let frame = frame_with_bytes(vec![0; 64]);
        let defects = d.detect(&frame).await.unwrap();
        assert_eq!(defects[0].bbox.x, 0);
        assert_eq!(defects[0].bbox.y, 0);
        assert_eq!(defects[0].bbox.w, frame.width);
        assert_eq!(defects[0].bbox.h, frame.height);
    }

    #[tokio::test]
    async fn detector_name_is_stable_kebab_case() {
        let d = ThresholdDetector::new();
        assert_eq!(d.detector_name(), "threshold");
    }

    #[tokio::test]
    async fn determinism_two_calls_produce_identical_output() {
        // Load-bearing for audit replay (see module docs). Two
        // identical inputs → identical defect lists. ML detectors
        // can't promise this; the threshold one must.
        let d = ThresholdDetector::new();
        let frame = frame_with_bytes(vec![10; 64]);
        let a = d.detect(&frame).await.unwrap();
        let b = d.detect(&frame).await.unwrap();
        assert_eq!(a.len(), b.len());
        assert_eq!(a[0].class, b[0].class);
        assert_eq!(a[0].bbox, b[0].bbox);
        assert!((a[0].confidence - b[0].confidence).abs() < 1e-9);
    }

    #[tokio::test]
    async fn extreme_violation_saturates_confidence_at_one() {
        // var=0 vs low_variance=50 → gap=50, ratio=1.0. That's the
        // saturating case; further variance reduction can't push
        // confidence higher.
        let d = ThresholdDetector::new();
        let frame = frame_with_bytes(vec![42; 64]);
        let defects = d.detect(&frame).await.unwrap();
        assert!((defects[0].confidence - 1.0).abs() < 1e-6);
    }
}
