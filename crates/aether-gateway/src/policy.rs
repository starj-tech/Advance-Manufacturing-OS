//! `BufferPolicy` — how a gateway handles bounded local buffering.
//!
//! ## Why three policies, not a tunable
//! Each policy maps to a distinct operational story:
//!   * `PassThrough` — cosmetic readouts where losing a sample
//!     during WAN outage is acceptable. Used for HMI gauges
//!     that show "current temp" but never need history.
//!   * `BufferUntilOnline` — compliance / audit / production
//!     counters where every sample matters. Bounded by
//!     `max_buffer`; the gateway surfaces a typed error when
//!     the cap is hit so the operator notices.
//!   * `DropOldestOnFull` — high-frequency telemetry where
//!     "latest reading wins" semantics are the right behavior
//!     (vibration sensors, flowmeters at >1 Hz). Bounded FIFO.
//!
//! Mixing semantics behind a single tunable would force every
//! caller to encode the policy as configuration; explicit
//! variants make the binding-time choice visible in code.

use serde::{Deserialize, Serialize};

/// Default buffer capacity per gateway. 4096 entries × ~256 B
/// average payload ≈ 1 MiB peak buffer — fits comfortably in
/// every industrial gateway's RAM (the cheapest Moxa UC has
/// 512 MiB) and covers ~4 hours of 1 Hz telemetry. Producers
/// that need more override via `MockGateway::with_capacity`.
pub const DEFAULT_BUFFER_CAPACITY: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BufferPolicy {
    /// No local buffering. Samples are forwarded immediately
    /// or dropped on WAN outage. Acceptable for cosmetic
    /// readouts that have no compliance / audit value.
    PassThrough,
    /// Accumulate samples until the WAN comes back, bounded by
    /// the gateway's capacity. Surfaces `BufferError::Full`
    /// when the cap is reached so the operator knows the
    /// outage exceeded buffered headroom — distinguishes
    /// "all data preserved" from "we started dropping."
    BufferUntilOnline,
    /// Bounded FIFO — oldest sample evicted when the buffer
    /// fills. Right policy for high-frequency telemetry where
    /// the latest reading wins (vibration, flowmeter > 1 Hz).
    DropOldestOnFull,
}

impl BufferPolicy {
    /// Kebab-case slug for the audit ledger.
    pub fn slug(&self) -> &'static str {
        match self {
            BufferPolicy::PassThrough => "pass-through",
            BufferPolicy::BufferUntilOnline => "buffer-until-online",
            BufferPolicy::DropOldestOnFull => "drop-oldest-on-full",
        }
    }

    /// Whether this policy will surface an error when the
    /// buffer fills, vs. silently evicting. Used by the
    /// healing layer: a full BufferUntilOnline is a healable
    /// fault; a full DropOldestOnFull is normal operation.
    pub fn errors_on_full(&self) -> bool {
        matches!(self, BufferPolicy::BufferUntilOnline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let policies = [
            BufferPolicy::PassThrough,
            BufferPolicy::BufferUntilOnline,
            BufferPolicy::DropOldestOnFull,
        ];
        let mut slugs: Vec<&'static str> = policies.iter().map(|p| p.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn errors_on_full_distinguishes_buffer_until_online_from_others() {
        // The healing layer treats this as the gate between
        // "operator alert needed" and "normal eviction."
        assert!(!BufferPolicy::PassThrough.errors_on_full());
        assert!(BufferPolicy::BufferUntilOnline.errors_on_full());
        assert!(!BufferPolicy::DropOldestOnFull.errors_on_full());
    }

    #[test]
    fn policy_round_trips_through_serde() {
        for p in [
            BufferPolicy::PassThrough,
            BufferPolicy::BufferUntilOnline,
            BufferPolicy::DropOldestOnFull,
        ] {
            let s = serde_json::to_string(&p).unwrap();
            let back: BufferPolicy = serde_json::from_str(&s).unwrap();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn default_capacity_is_reasonable_for_industrial_gateways() {
        // Pin the constant as a documentation anchor.
        assert_eq!(DEFAULT_BUFFER_CAPACITY, 4096);
    }
}
