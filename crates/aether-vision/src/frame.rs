//! Captured frame from a [`crate::Camera`] — bytes + identity +
//! metadata.
//!
//! ## Wire shape
//! `Frame` is `Serialize + Deserialize` so it can travel through
//! the sync outbox once V10's `actuator_commands` migration lands.
//! For sensitive frames the `bytes` field is replaced with the
//! sealed envelope output before persistence — `aether-crypto::
//! envelope::seal(dek, frame.bytes)` produces the same `Vec<u8>`
//! shape, just opaque.
//!
//! ## Why a separate `FrameId` rather than reusing the actuator
//! permit id
//! One Capture command can emit multiple frames in burst mode in
//! future. Tying frames to permits 1:1 would force a new permit per
//! burst-frame — not workable. `FrameId` is independent and stable;
//! the audit row links permit → frame_id(s).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use aether_core::CameraId;

use crate::privacy::PrivacyClass;

/// Stable per-frame identifier. UUIDv7 so frame ids sort
/// chronologically without a separate timestamp index. Newtype so
/// the type system rejects `MachineId` where a `FrameId` is wanted.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameId(pub Uuid);

impl FrameId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for FrameId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FrameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// On-wire pixel format. Internal tag (`encoding`) keeps the JSON
/// readable for Edge Function consumers; payload-bearing variants
/// (`Jpeg`, `Raw`) carry the per-format knobs inline.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "encoding", rename_all = "kebab-case")]
pub enum PixelFormat {
    /// Lossless 8/16-bit per channel. Used when defect detection
    /// needs sharp edges (scratches, dimensional checks).
    Png,
    /// Lossy. `quality` 0..=100; production typically uses 85
    /// (visible quality matches PNG within 1% on most factory
    /// scenes; ~10x smaller).
    Jpeg { quality: u8 },
    /// Uncompressed raw pixels. `channels` = 1 (grayscale) or 3
    /// (RGB); `bit_depth` = 8 or 16. Used when an inference model
    /// expects a specific layout (PNG round-trips can corrupt the
    /// channel order on some cameras).
    Raw { channels: u8, bit_depth: u8 },
}

impl PixelFormat {
    /// Human-readable slug for logs / metrics labels.
    pub fn slug(&self) -> &'static str {
        match self {
            PixelFormat::Png => "png",
            PixelFormat::Jpeg { .. } => "jpeg",
            PixelFormat::Raw { .. } => "raw",
        }
    }
}

/// One captured frame. `bytes` carries the encoded image (for `Png`/
/// `Jpeg`) or the raw pixel buffer (for `Raw`). Size is bounded by
/// the camera's configured resolution and format — not by anything
/// here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub id: FrameId,
    pub camera_id: CameraId,
    pub captured_at: DateTime<Utc>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// Inherited from the camera at capture time. Stays attached to
    /// the frame so a downstream consumer can't accidentally
    /// downgrade a Sensitive frame to Public storage by losing the
    /// camera context.
    pub privacy: PrivacyClass,
    /// Encoded image bytes. For `PrivacyClass::Sensitive` frames the
    /// pipeline replaces this with a sealed envelope output before
    /// any durable write.
    pub bytes: Vec<u8>,
}

impl Frame {
    /// Convenience: byte-count for the encoded image. Used by tests
    /// and metric labels; production cares about it for backpressure
    /// decisions.
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_ids_are_unique_per_construction() {
        // Distinct ids matter for the actuator_commands audit lookup;
        // collisions would break per-frame retrieval.
        let a = FrameId::new();
        let b = FrameId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn frame_id_sorts_chronologically_via_uuidv7() {
        // UUIDv7 embeds a millisecond timestamp in its high bits. Two
        // ids minted in order should compare in order — that's how
        // audit logs ordered by frame_id stay chronological without
        // a separate timestamp index.
        let a = FrameId::new();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = FrameId::new();
        // Compare as UUIDs (not the wrapper, which has no Ord).
        assert!(a.into_uuid() < b.into_uuid());
    }

    #[test]
    fn pixel_format_slugs_distinct_and_kebab_cased() {
        let formats = [
            PixelFormat::Png,
            PixelFormat::Jpeg { quality: 85 },
            PixelFormat::Raw {
                channels: 3,
                bit_depth: 8,
            },
        ];
        let mut slugs: Vec<&'static str> = formats.iter().map(|f| f.slug()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), 3);
        for s in &slugs {
            assert!(!s.contains('_'));
        }
    }

    #[test]
    fn jpeg_quality_round_trips_through_serde() {
        // Internal tag means the wire shape is
        // `{"encoding":"jpeg","quality":85}` — pin so the field
        // name doesn't drift.
        let f = PixelFormat::Jpeg { quality: 85 };
        let s = serde_json::to_string(&f).unwrap();
        assert!(s.contains("\"encoding\":\"jpeg\""));
        assert!(s.contains("\"quality\":85"));
        let back: PixelFormat = serde_json::from_str(&s).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn raw_format_carries_channels_and_bit_depth() {
        let f = PixelFormat::Raw {
            channels: 1,
            bit_depth: 16,
        };
        let s = serde_json::to_string(&f).unwrap();
        let back: PixelFormat = serde_json::from_str(&s).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn frame_byte_len_matches_bytes_vector() {
        let frame = Frame {
            id: FrameId::new(),
            camera_id: CameraId::new(),
            captured_at: Utc::now(),
            width: 640,
            height: 480,
            format: PixelFormat::Png,
            privacy: PrivacyClass::Public,
            bytes: vec![0u8; 1024],
        };
        assert_eq!(frame.byte_len(), 1024);
    }

    #[test]
    fn frame_round_trips_through_serde_with_privacy_attached() {
        // Privacy stays attached through ser/de — a sensitive frame
        // can't be silently demoted to public by going through JSON.
        let original = Frame {
            id: FrameId::new(),
            camera_id: CameraId::new(),
            captured_at: Utc::now(),
            width: 640,
            height: 480,
            format: PixelFormat::Png,
            privacy: PrivacyClass::Sensitive,
            bytes: vec![1, 2, 3, 4],
        };
        let s = serde_json::to_string(&original).unwrap();
        let back: Frame = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, original.id);
        assert_eq!(back.privacy, PrivacyClass::Sensitive);
        assert_eq!(back.bytes, original.bytes);
    }
}
