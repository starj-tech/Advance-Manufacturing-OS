//! SOS event lifecycle state machine.
//!
//! `sos.rs` broadcasts an emergency; this governs what happens to that
//! event afterwards. The `sos_events.status` column allows four states —
//! `active`, `acknowledged`, `resolved`, `false-alarm` — but nothing
//! enforced the legal transitions between them. This does, so a responder
//! UI / server function can't (e.g.) resolve an alarm nobody acknowledged
//! or reopen a closed one.
//!
//! ## Transitions
//! ```text
//!            acknowledge        resolve
//!   Active ──────────────▶ Acknowledged ──────────▶ Resolved (terminal)
//!     │                        │
//!     │ false-alarm            │ false-alarm
//!     ▼                        ▼
//!   FalseAlarm (terminal) ◀────┘
//! ```
//! An emergency must be acknowledged by a responder before it can be
//! resolved — closing an unacknowledged alarm as "handled" would let a
//! real emergency be silently dismissed. False-alarm is reachable from
//! either open state. Resolved and FalseAlarm are terminal.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SosStatus {
    Active,
    Acknowledged,
    Resolved,
    FalseAlarm,
}

/// Operator/responder actions that drive an SOS event forward.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SosTransition {
    /// A responder has seen the alarm and is on it.
    Acknowledge,
    /// The situation is handled.
    Resolve,
    /// The alarm was not a real emergency.
    MarkFalseAlarm,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SosLifecycleError {
    #[error("cannot {action:?} an SOS event in state {from:?}")]
    InvalidTransition {
        from: SosStatus,
        action: SosTransition,
    },
}

impl SosStatus {
    /// Terminal states accept no further transitions.
    pub fn is_terminal(self) -> bool {
        matches!(self, SosStatus::Resolved | SosStatus::FalseAlarm)
    }

    /// Apply a transition, returning the next state or rejecting an
    /// illegal move. Pure: no clock, no I/O — the caller persists the
    /// result and timestamps it.
    pub fn apply(self, action: SosTransition) -> Result<SosStatus, SosLifecycleError> {
        use SosStatus::*;
        use SosTransition::*;
        let next = match (self, action) {
            (Active, Acknowledge) => Acknowledged,
            (Active, MarkFalseAlarm) => FalseAlarm,
            (Acknowledged, Resolve) => Resolved,
            (Acknowledged, MarkFalseAlarm) => FalseAlarm,
            (from, action) => return Err(SosLifecycleError::InvalidTransition { from, action }),
        };
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::SosStatus::*;
    use super::SosTransition::*;
    use super::*;

    #[test]
    fn happy_path_active_ack_resolve() {
        let s = Active.apply(Acknowledge).unwrap();
        assert_eq!(s, Acknowledged);
        let s = s.apply(Resolve).unwrap();
        assert_eq!(s, Resolved);
        assert!(s.is_terminal());
    }

    #[test]
    fn cannot_resolve_without_acknowledging() {
        assert_eq!(
            Active.apply(Resolve),
            Err(SosLifecycleError::InvalidTransition {
                from: Active,
                action: Resolve
            })
        );
    }

    #[test]
    fn false_alarm_from_either_open_state() {
        assert_eq!(Active.apply(MarkFalseAlarm).unwrap(), FalseAlarm);
        assert_eq!(Acknowledged.apply(MarkFalseAlarm).unwrap(), FalseAlarm);
    }

    #[test]
    fn terminal_states_reject_everything() {
        for terminal in [Resolved, FalseAlarm] {
            assert!(terminal.is_terminal());
            for action in [Acknowledge, Resolve, MarkFalseAlarm] {
                assert!(terminal.apply(action).is_err());
            }
        }
    }

    #[test]
    fn cannot_double_acknowledge() {
        // Acknowledged + Acknowledge is not a legal move.
        assert!(Acknowledged.apply(Acknowledge).is_err());
    }

    #[test]
    fn status_serde_matches_db_strings() {
        assert_eq!(serde_json::to_string(&Active).unwrap(), "\"active\"");
        assert_eq!(
            serde_json::to_string(&FalseAlarm).unwrap(),
            "\"false-alarm\""
        );
        let back: SosStatus = serde_json::from_str("\"acknowledged\"").unwrap();
        assert_eq!(back, Acknowledged);
    }
}
