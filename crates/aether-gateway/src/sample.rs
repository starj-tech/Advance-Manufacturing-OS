//! `BufferedSample` — one telemetry record waiting to be
//! forwarded upstream.
//!
//! ## Why provenance fields live here
//! The V10 sync layer reconstructs ordering after a multi-hour
//! WAN outage by replaying buffered samples in their original
//! sequence. That requires three things on every record:
//!   1. `topic` — the upstream destination (MQTT topic, Kafka
//!      stream, REST endpoint). Pinned at buffer-time so a
//!      gateway reconfig mid-buffer doesn't misroute samples
//!      already in the queue.
//!   2. `payload` — opaque bytes (the gateway doesn't interpret
//!      content; the upstream consumer parses).
//!   3. `buffered_at` — wall-time at the gateway when it
//!      accepted the sample. Used for HLC ordering at the
//!      upstream apply path. NOT the sensor read time; that's
//!      inside the payload if the sensor stamped it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Soft upper bound on payload bytes. 64 KiB covers every
/// industrial telemetry shape we've seen in pilots (most are
/// <1 KiB); larger blobs (video frames, ML model weights) take
/// the V4 `aether-vision` sealed-frame path instead.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BufferedSample {
    /// Upstream destination — pinned at buffer-time so a
    /// gateway reconfig mid-buffer doesn't misroute already-
    /// queued samples.
    pub topic: String,
    /// Opaque payload bytes. The gateway doesn't interpret
    /// content; upstream consumers parse.
    pub payload: Vec<u8>,
    /// Wall-time the gateway accepted the sample. Source of
    /// truth for upstream HLC ordering.
    pub buffered_at: DateTime<Utc>,
}

impl BufferedSample {
    /// Size of the encoded payload in bytes — recorded in the
    /// audit row so a downstream analyst can answer "how much
    /// did we buffer during the outage" without scanning
    /// payloads.
    pub fn payload_size(&self) -> usize {
        self.payload.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_round_trips_through_serde() {
        let s = BufferedSample {
            topic: "factory/line-1/temp".into(),
            payload: vec![0x01, 0x02, 0x03],
            buffered_at: Utc::now(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: BufferedSample = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn payload_size_matches_byte_count() {
        let s = BufferedSample {
            topic: "x".into(),
            payload: vec![0u8; 1024],
            buffered_at: Utc::now(),
        };
        assert_eq!(s.payload_size(), 1024);
    }

    #[test]
    fn max_payload_bytes_is_reasonable_for_industrial_telemetry() {
        // Pin the cap as a documentation anchor. 64 KiB is
        // 32x the largest payload we've observed across pilots
        // (a Modbus scan of a 1000-register bank), so the cap
        // is generous but bounded.
        assert_eq!(MAX_PAYLOAD_BYTES, 65_536);
    }
}
