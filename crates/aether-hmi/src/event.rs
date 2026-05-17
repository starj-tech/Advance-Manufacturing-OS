//! `OperatorEvent` — typed inbound interactions from an HMI
//! surface.
//!
//! ## Why a closed enum
//! The interaction taxonomy is small and stable: tap, swipe,
//! voice utterance, gaze-dwell, generic Acknowledge / Cancel.
//! A closed enum forces every handler to match exhaustively
//! (so a new event variant is a deliberate workspace change),
//! and the kebab-case slug is persisted into the audit ledger
//! so an analyst can answer "how many critical alarms got
//! Acknowledged vs Cancelled this shift" with a typed filter.
//!
//! ## What's NOT here
//! Pointer-move events, continuous drag deltas, eye-tracking
//! sample streams. Those are sub-100ms cadence and would
//! dominate the audit ledger without operational value — they
//! belong on a separate gesture-stream bus that doesn't land
//! in audit. Layer that on top in a future session if a
//! customer's workflow needs it.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Direction for `Swipe` events. The four cardinal directions
/// cover every gesture-driven workflow we've seen — diagonals
/// are too easy to misclassify on a low-cost touch panel and
/// haven't shown up in any pilot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum OperatorEvent {
    /// Single tap / press on a named region. `region` is the
    /// HMI's logical area name ("home", "alarm-clear", "menu-
    /// open"). Layout coordinates are out of scope — the surface
    /// hands us the resolved region.
    Tap { region: String, at: DateTime<Utc> },
    /// Swipe gesture on the surface. `region` is the originating
    /// area; `direction` is one of the four cardinals.
    Swipe {
        region: String,
        direction: SwipeDirection,
        at: DateTime<Utc>,
    },
    /// Voice utterance — already decoded to text by the surface's
    /// ASR (the surface owns the ASR config). `confidence` is
    /// the ASR confidence 0..=1 where the vendor exposes one.
    Voice {
        utterance: String,
        confidence: Option<f32>,
        at: DateTime<Utc>,
    },
    /// Operator's gaze rested on a region for at least
    /// `duration_ms` (smart-glasses gesture). Used as a hands-
    /// free selection signal. Surfaces without gaze tracking
    /// never emit this — the HmiKind helper filters at routing
    /// time.
    GazeDwell {
        region: String,
        duration_ms: u32,
        at: DateTime<Utc>,
    },
    /// Generic positive acknowledgement. Maps to the green
    /// "OK"/"Confirm" button across every surface — the
    /// surface translates its hardware button to this variant.
    Acknowledge { at: DateTime<Utc> },
    /// Generic negative dismissal. Red "Cancel"/"Dismiss" /
    /// pendant 0-key.
    Cancel { at: DateTime<Utc> },
}

impl OperatorEvent {
    /// Kebab-case discriminator slug. Persisted into the audit
    /// ledger so an analyst can filter event types without
    /// parsing the structured payload.
    pub fn slug(&self) -> &'static str {
        match self {
            OperatorEvent::Tap { .. } => "tap",
            OperatorEvent::Swipe { .. } => "swipe",
            OperatorEvent::Voice { .. } => "voice",
            OperatorEvent::GazeDwell { .. } => "gaze-dwell",
            OperatorEvent::Acknowledge { .. } => "acknowledge",
            OperatorEvent::Cancel { .. } => "cancel",
        }
    }

    /// Wall-time the event was emitted. Used for HLC ordering
    /// when the event flows into the V10 outbox.
    pub fn at(&self) -> DateTime<Utc> {
        match self {
            OperatorEvent::Tap { at, .. } => *at,
            OperatorEvent::Swipe { at, .. } => *at,
            OperatorEvent::Voice { at, .. } => *at,
            OperatorEvent::GazeDwell { at, .. } => *at,
            OperatorEvent::Acknowledge { at } => *at,
            OperatorEvent::Cancel { at } => *at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_distinct_and_kebab_cased_across_all_variants() {
        let now = Utc::now();
        let cases = [
            OperatorEvent::Tap {
                region: "x".into(),
                at: now,
            },
            OperatorEvent::Swipe {
                region: "x".into(),
                direction: SwipeDirection::Up,
                at: now,
            },
            OperatorEvent::Voice {
                utterance: "x".into(),
                confidence: None,
                at: now,
            },
            OperatorEvent::GazeDwell {
                region: "x".into(),
                duration_ms: 500,
                at: now,
            },
            OperatorEvent::Acknowledge { at: now },
            OperatorEvent::Cancel { at: now },
        ];
        let mut slugs: Vec<&'static str> = cases.iter().map(|e| e.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before, "all slugs distinct");
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn tap_event_round_trips_through_serde_with_kind_tag() {
        let now = Utc::now();
        let e = OperatorEvent::Tap {
            region: "alarm-clear".into(),
            at: now,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("\"kind\":\"tap\""));
        let back: OperatorEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn voice_confidence_is_optional_for_asr_without_score() {
        let now = Utc::now();
        let e = OperatorEvent::Voice {
            utterance: "halt".into(),
            confidence: None,
            at: now,
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: OperatorEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn swipe_direction_serde_uses_kebab_case() {
        for d in [
            SwipeDirection::Up,
            SwipeDirection::Down,
            SwipeDirection::Left,
            SwipeDirection::Right,
        ] {
            let s = serde_json::to_string(&d).unwrap();
            let back: SwipeDirection = serde_json::from_str(&s).unwrap();
            assert_eq!(back, d);
        }
    }

    #[test]
    fn at_returns_event_timestamp_for_every_variant() {
        // Pin that every variant exposes the wall-time the
        // surface stamped — used by the V10 outbox encoder.
        let now = Utc::now();
        for e in [
            OperatorEvent::Tap {
                region: "x".into(),
                at: now,
            },
            OperatorEvent::Swipe {
                region: "x".into(),
                direction: SwipeDirection::Up,
                at: now,
            },
            OperatorEvent::Voice {
                utterance: "x".into(),
                confidence: None,
                at: now,
            },
            OperatorEvent::GazeDwell {
                region: "x".into(),
                duration_ms: 500,
                at: now,
            },
            OperatorEvent::Acknowledge { at: now },
            OperatorEvent::Cancel { at: now },
        ] {
            assert_eq!(e.at(), now);
        }
    }
}
