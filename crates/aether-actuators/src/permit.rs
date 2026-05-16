//! `ActuatorPermit` — single-use, TTL-bounded token that proves the
//! safety interlock approved the about-to-dispatch command.
//!
//! ## The invariant we are enforcing
//! Every actuator command must pass [`aether_safety::interlock::evaluate`]
//! immediately before dispatch. A naïve implementation puts the
//! `evaluate()` call inside each [`crate::Actuator::dispatch`] impl
//! — but that's distributed responsibility and easy to forget when
//! a new Actuator type is added. We do the opposite: make the
//! permit a type-system precondition on `dispatch`, and make the
//! permit's only constructor go through the interlock.
//!
//! ## Three properties
//! 1. **Type-level gate**: `ActuatorPermit` has a private constructor.
//!    The only public way to build one is via [`crate::gate::gate`],
//!    which requires an `UnlockRequest` and returns either a permit
//!    or the deny verdict.
//! 2. **Single-use**: a permit consumed by one `dispatch` cannot be
//!    reused. Operator double-tapping "fire" — second attempt sees
//!    [`crate::actuator::ActuatorError::PermitConsumed`].
//! 3. **Time-bounded**: every permit carries an `expires_at` instant.
//!    A permit issued at T but dispatched at T+10s with TTL=5s is
//!    rejected — protects against the user pressing "fire" then
//!    walking away while the operation queues.
//!
//! ## Why a `Cell<bool>` for the consumed flag
//! `ActuatorPermit` is `Send` (Actuator impls are `Send + Sync`) but
//! consumption mutates the flag exactly once. We use
//! `std::sync::atomic::AtomicBool` so the flag is visible across
//! threads without a `Mutex` — the dispatcher might own the permit
//! on one task and the actuator dispatch path on another.

use chrono::{DateTime, Duration, Utc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Default permit time-to-live. 5 seconds is long enough for a user
/// click → backend dispatch → hardware receive on a healthy network,
/// short enough that an operator who walks away after pressing "fire"
/// doesn't accidentally arm a stale permit.
pub const DEFAULT_PERMIT_TTL_MS: i64 = 5_000;

pub static DEFAULT_PERMIT_TTL: PermitTtl = PermitTtl(DEFAULT_PERMIT_TTL_MS);

/// Permit TTL in milliseconds. Wrapping `i64` so values can be reused
/// across `chrono::Duration::milliseconds` without a unit ambiguity.
#[derive(Clone, Copy, Debug)]
pub struct PermitTtl(pub i64);

impl PermitTtl {
    pub fn duration(&self) -> Duration {
        Duration::milliseconds(self.0)
    }
}

/// Single-use, TTL-bounded approval token for one actuator command.
/// Constructed only via [`crate::gate::gate`] (private constructor
/// trick: the inner field is non-`pub` and `new_unchecked` is
/// `pub(crate)`, so external code cannot synthesize a permit).
#[derive(Clone, Debug)]
pub struct ActuatorPermit {
    permit_id: Uuid,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    consumed: Arc<AtomicBool>,
}

impl ActuatorPermit {
    /// Crate-private constructor. External code must go through
    /// [`crate::gate::gate`]. Tests inside this crate use this path
    /// to construct permits without standing up an interlock.
    pub(crate) fn new_unchecked(ttl: PermitTtl) -> Self {
        let now = Utc::now();
        Self {
            permit_id: Uuid::new_v4(),
            issued_at: now,
            expires_at: now + ttl.duration(),
            consumed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn permit_id(&self) -> Uuid {
        self.permit_id
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// `true` if the permit has been spent (by a successful or failed
    /// `dispatch` attempt). Spent permits can never be re-used.
    pub fn is_consumed(&self) -> bool {
        self.consumed.load(Ordering::SeqCst)
    }

    /// `true` if `now` is past the permit's expiry.
    pub fn is_expired_at(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    /// Mark the permit consumed. Atomic compare-and-swap so a
    /// concurrent double-dispatch can be detected: the loser receives
    /// `false` and returns `PermitConsumed` to its caller.
    ///
    /// Returns `true` exactly once across all callers; every
    /// subsequent invocation (same thread or different) returns
    /// `false`.
    pub fn try_consume(&self) -> bool {
        self.consumed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn fresh_permit_is_neither_consumed_nor_expired() {
        let p = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        assert!(!p.is_consumed());
        assert!(!p.is_expired_at(Utc::now()));
    }

    #[test]
    fn try_consume_succeeds_once_and_fails_thereafter() {
        let p = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        assert!(p.try_consume());
        assert!(p.is_consumed());
        // Second attempt — the load-bearing rejection that prevents
        // an operator's double-tap from firing the same command twice.
        assert!(!p.try_consume());
    }

    #[test]
    fn concurrent_consume_only_one_thread_wins() {
        // The CAS makes this race well-defined: exactly one thread
        // sees `true`; every other gets `false`. Without atomics
        // we'd risk both threads "winning" and dispatching twice.
        let p = Arc::new(ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL));
        let mut handles = Vec::new();
        let winners = Arc::new(AtomicBool::new(false));

        for _ in 0..8 {
            let p = p.clone();
            let winners = winners.clone();
            handles.push(thread::spawn(move || {
                if p.try_consume() {
                    // First successful consume sets the witness.
                    // CAS makes a second-winner unreachable, so the
                    // swap below must observe `false`.
                    let was = winners.swap(true, Ordering::SeqCst);
                    assert!(!was, "more than one thread won the CAS");
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(p.is_consumed());
    }

    #[test]
    fn expired_permit_reports_expired_at_check_time() {
        let p = ActuatorPermit::new_unchecked(PermitTtl(50)); // 50 ms TTL
        thread::sleep(std::time::Duration::from_millis(80));
        assert!(p.is_expired_at(Utc::now()));
    }

    #[test]
    fn permit_ids_are_unique_across_constructions() {
        // A permit's id ends up in the audit log; collisions would
        // break the per-command query path.
        let a = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let b = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        assert_ne!(a.permit_id(), b.permit_id());
    }

    #[test]
    fn ttl_duration_matches_milliseconds_input() {
        let t = PermitTtl(2_500);
        assert_eq!(t.duration(), Duration::milliseconds(2_500));
    }
}
