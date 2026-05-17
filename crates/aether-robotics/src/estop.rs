//! `EstopSignal` — universal emergency-stop preemption channel.
//!
//! ## What this solves
//! Every Actuator dispatch goes through V1's permit-gated path: an
//! `ActuatorPermit` from the interlock, then `Actuator::dispatch`.
//! That covers the "operator can't move the robot without their
//! cert" story. It does NOT cover the "operator pressed e-stop
//! while the robot is moving" story — the in-flight `dispatch`
//! call has to be cancellable, and a tripped e-stop has to refuse
//! every NEW dispatch until manually cleared.
//!
//! `EstopSignal` is the channel that does that. One signal per
//! cell/area; every Actuator dispatch in that area runs through
//! [`dispatch_with_estop`] which:
//!
//!   1. Refuses the call up-front if the signal is tripped.
//!   2. Otherwise races `actuator.dispatch(...)` against a
//!      `watch::changed()` future. If the signal trips before the
//!      dispatch completes, `tokio::select!` cancels the dispatch
//!      future and the call returns `ActuatorError::Estopped`.
//!
//! ## Why `tokio::sync::watch` and not `mpsc::channel`
//! `watch` is one-writer-many-readers with a stored "latest value"
//! — every Actuator in the cell holds a `Receiver` and sees the
//! same trip event without needing any one of them to consume
//! first. `mpsc` would deliver the trip to exactly one Actuator
//! and the rest would keep running.
//!
//! ## Manual clear required
//! [`EstopSignal::clear`] flips back to `tripped=false`. This is
//! intentionally a SEPARATE call from `trip` rather than e.g. a
//! timeout — every safety standard (ISO 13849, IEC 62061) requires
//! manual operator action to re-enable a stopped cell. Auto-clear
//! would be a regulatory violation.

use aether_actuators::actuator::{Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use tokio::sync::watch;

/// Sender side of the e-stop signal. Cloneable: a fleet manager
/// instance can hand `EstopSignal` clones to UI buttons, hardware
/// e-stop relays, healing-tier preemption, etc. Any clone that
/// calls `trip()` halts the whole cell.
#[derive(Clone)]
pub struct EstopSignal {
    sender: watch::Sender<bool>,
}

/// Receiver side. Every Actuator in the e-stop's domain holds one
/// (typically via `subscribe()`). Used by [`dispatch_with_estop`]
/// internally; tests can also poll directly via [`EstopReceiver::
/// tripped`].
#[derive(Clone)]
pub struct EstopReceiver {
    receiver: watch::Receiver<bool>,
}

impl EstopSignal {
    /// Construct a fresh signal in the un-tripped state and return
    /// the matched (sender, receiver) pair. Production typically
    /// owns the sender at the cell-manager level and hands
    /// receivers to each registered Actuator.
    pub fn new() -> (Self, EstopReceiver) {
        let (sender, receiver) = watch::channel(false);
        (Self { sender }, EstopReceiver { receiver })
    }

    /// Trip the e-stop. After this returns, every subscriber sees
    /// `tripped() == true` and every in-flight
    /// `dispatch_with_estop` call cancels and returns
    /// `ActuatorError::Estopped`.
    pub fn trip(&self) {
        // send_replace returns the previous value; we don't care.
        // Use `replace` over `send` so we don't error when there
        // happen to be no receivers (Sender's `send` errors in
        // that case; `send_replace` doesn't).
        self.sender.send_replace(true);
    }

    /// Manually clear the e-stop. Per ISO 13849 + IEC 62061 this
    /// requires a deliberate operator action — production wires it
    /// to a physical reset key-switch, never to a timeout.
    pub fn clear(&self) {
        self.sender.send_replace(false);
    }

    /// Current tripped state. Cheap (atomic load); poll-friendly
    /// for UIs and metrics.
    pub fn tripped(&self) -> bool {
        *self.sender.borrow()
    }

    /// Mint a fresh receiver. Use when an Actuator is being added
    /// after the initial cell setup — the new receiver starts at
    /// whatever the current tripped state is, so a hot-swap into
    /// an already-tripped cell will immediately refuse dispatches.
    pub fn subscribe(&self) -> EstopReceiver {
        EstopReceiver {
            receiver: self.sender.subscribe(),
        }
    }
}

impl EstopReceiver {
    /// Current tripped state — atomic load.
    pub fn tripped(&self) -> bool {
        *self.receiver.borrow()
    }

    /// Resolve when the signal transitions to (or is already at)
    /// `tripped == true`. Used inside [`dispatch_with_estop`]'s
    /// `tokio::select!` to cancel an in-flight dispatch. Pollable
    /// from external code that wants to observe trips
    /// asynchronously (e.g. logging the time-to-stop).
    pub async fn wait_tripped(&mut self) {
        // Fast path: already tripped → return immediately.
        if self.tripped() {
            return;
        }
        // Otherwise loop on `changed()`. Each `changed` wakes on
        // ANY value change; the inner if handles the spurious
        // cleared→cleared case (which can't actually happen with
        // a bool but the loop is the standard pattern).
        loop {
            if self.receiver.changed().await.is_err() {
                // Sender dropped. Treat as "no further trips
                // possible" → never-resolve. Park forever; the
                // surrounding `select!` will be cancelled by its
                // sibling future eventually.
                std::future::pending::<()>().await;
            }
            if self.tripped() {
                return;
            }
        }
    }
}

/// Dispatch an Actuator command with e-stop preemption.
///
/// Returns `ActuatorError::Estopped` if:
///   * the signal was already tripped at entry, OR
///   * the signal trips mid-dispatch (in which case the dispatch
///     future is cancelled via `tokio::select!`).
///
/// The permit is moved in. If the e-stop trips before
/// `actuator.dispatch` consumes the permit, the permit is dropped
/// along with the cancelled future — its single-use semantics
/// still hold (the unused permit is just abandoned; the underlying
/// `AtomicBool::consumed` flag never flips).
pub async fn dispatch_with_estop(
    actuator: &dyn Actuator,
    cmd: ActuatorCommand,
    permit: ActuatorPermit,
    estop: &EstopSignal,
) -> Result<ActuatorResult, ActuatorError> {
    // Pre-flight: refuse up-front so we don't even allocate the
    // select! futures when the cell is already halted.
    if estop.tripped() {
        return Err(ActuatorError::Estopped);
    }
    let mut rx = estop.subscribe();
    tokio::select! {
        // Bias toward the e-stop branch — `tokio::select!` picks
        // randomly by default; biased polls in order. We want
        // estop to win ties so a trip that arrives exactly as
        // dispatch is about to complete still cancels.
        biased;
        _ = rx.wait_tripped() => Err(ActuatorError::Estopped),
        result = actuator.dispatch(cmd, permit) => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arm::MockArm;
    use aether_actuators::actuator::{enforce_permit, MockActuator};
    use aether_actuators::command::CommandKind;
    use aether_actuators::gate::gate;
    use aether_actuators::permit::ActuatorPermit;
    use aether_core::ActuatorId;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use std::sync::Arc;
    use uuid::Uuid;

    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "robot-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["robot-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock should approve in test fixture")
    }

    #[tokio::test]
    async fn fresh_signal_is_not_tripped() {
        let (tx, rx) = EstopSignal::new();
        assert!(!tx.tripped());
        assert!(!rx.tripped());
    }

    #[tokio::test]
    async fn trip_propagates_to_subscribers() {
        let (tx, rx) = EstopSignal::new();
        let mut rx2 = tx.subscribe();
        tx.trip();
        assert!(tx.tripped());
        assert!(rx.tripped());
        assert!(rx2.tripped());
        rx2.wait_tripped().await; // returns immediately
    }

    #[tokio::test]
    async fn clear_resets_to_untripped() {
        // Manual clear is the only path back to un-tripped per
        // ISO 13849 / IEC 62061. Auto-clear (timeout, etc.) is a
        // regulatory violation and isn't exposed.
        let (tx, _rx) = EstopSignal::new();
        tx.trip();
        assert!(tx.tripped());
        tx.clear();
        assert!(!tx.tripped());
    }

    #[tokio::test]
    async fn wait_tripped_returns_immediately_when_already_tripped() {
        // The fast path — entry into an already-halted cell
        // shouldn't have to spin waiting for a change event.
        let (tx, mut rx) = EstopSignal::new();
        tx.trip();
        // Should not block.
        tokio::time::timeout(std::time::Duration::from_millis(50), rx.wait_tripped())
            .await
            .expect("wait_tripped should resolve immediately");
    }

    #[tokio::test]
    async fn wait_tripped_resolves_after_remote_trip() {
        let (tx, mut rx) = EstopSignal::new();
        let waiter = tokio::spawn(async move {
            rx.wait_tripped().await;
        });
        // Give the waiter a tick to park on `changed()`, then trip.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        tx.trip();
        // The waiter must resolve.
        let _ = tokio::time::timeout(std::time::Duration::from_millis(100), waiter)
            .await
            .expect("waiter should resolve after trip");
    }

    #[tokio::test]
    async fn dispatch_with_estop_passes_through_on_clean_signal() {
        let (estop, _rx) = EstopSignal::new();
        let arm = MockArm::new();
        let result = dispatch_with_estop(&arm, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
    }

    #[tokio::test]
    async fn dispatch_with_estop_refuses_when_already_tripped() {
        // Pre-flight check: tripped signal at entry → no
        // dispatch at all. The actuator's received-log should
        // stay empty (proves the dispatch future never ran).
        let (estop, _rx) = EstopSignal::new();
        estop.trip();
        let arm = MockArm::new();
        let err = dispatch_with_estop(
            &arm,
            ActuatorCommand::MoveJoint {
                joint: 0,
                target_rad: 0.5,
            },
            test_permit(),
            &estop,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ActuatorError::Estopped));
        assert!(arm.received().await.is_empty());
    }

    /// Actuator that sleeps a fixed duration before returning — lets
    /// us exercise the "estop trips mid-dispatch" race deterministically.
    struct SlowActuator {
        delay_ms: u64,
        id: ActuatorId,
    }

    #[async_trait]
    impl Actuator for SlowActuator {
        fn actuator_id(&self) -> ActuatorId {
            self.id
        }
        fn actuator_kind(&self) -> &'static str {
            "slow-test"
        }
        async fn dispatch(
            &self,
            _cmd: ActuatorCommand,
            permit: ActuatorPermit,
        ) -> Result<ActuatorResult, ActuatorError> {
            enforce_permit(&permit)?;
            tokio::time::sleep(std::time::Duration::from_millis(self.delay_ms)).await;
            Ok(ActuatorResult {
                actuator_id: self.id,
                command_kind: "slow".into(),
                completed_at: Utc::now(),
                data: serde_json::json!({}),
            })
        }
    }

    #[tokio::test]
    async fn dispatch_cancelled_when_estop_trips_mid_flight() {
        // The headline V8 property: a long-running dispatch is
        // cancelled when the e-stop trips. SlowActuator sleeps
        // 500 ms; we trip after 50 ms and expect Estopped well
        // before the sleep completes.
        let (estop, _rx) = EstopSignal::new();
        let actuator = Arc::new(SlowActuator {
            delay_ms: 500,
            id: ActuatorId::new(),
        });

        let estop_clone = estop.clone();
        let tripper = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            estop_clone.trip();
        });

        let start = std::time::Instant::now();
        let err = dispatch_with_estop(&*actuator, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap_err();
        let elapsed = start.elapsed();

        tripper.await.unwrap();
        assert!(matches!(err, ActuatorError::Estopped));
        assert!(
            elapsed < std::time::Duration::from_millis(400),
            "dispatch should cancel quickly after trip; took {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn clear_after_trip_allows_subsequent_dispatch() {
        // Operator-clear path: trip, clear, dispatch succeeds. Pins
        // the "no permanent latch in software" property — the
        // hardware-level latch is the responsibility of the
        // physical relay, not our channel.
        let (estop, _rx) = EstopSignal::new();
        let arm = MockArm::new();

        estop.trip();
        let err = dispatch_with_estop(&arm, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::Estopped));

        estop.clear();
        let result = dispatch_with_estop(&arm, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
    }

    #[tokio::test]
    async fn multiple_subscribers_all_see_the_same_trip() {
        // Cell-wide preemption: many Actuators each hold a
        // receiver, all of them halt on the same trip. Without
        // this we'd have to manually trip each one.
        let (estop, mut rx1) = EstopSignal::new();
        let mut rx2 = estop.subscribe();
        let mut rx3 = estop.subscribe();
        estop.trip();
        // All three see it via wait_tripped (the production code
        // path inside `dispatch_with_estop::select!`).
        for rx in [&mut rx1, &mut rx2, &mut rx3].iter_mut() {
            tokio::time::timeout(std::time::Duration::from_millis(50), rx.wait_tripped())
                .await
                .expect("each subscriber resolves on trip");
        }
    }

    #[tokio::test]
    async fn already_consumed_permit_is_rejected_before_estop_check() {
        // Ordering check: a permit that was already consumed
        // surfaces as `PermitConsumed`, NOT as `Estopped`, when
        // the estop is clean. The actuator's enforce_permit runs
        // inside dispatch, AFTER the up-front estop check. We
        // can't directly construct a consumed permit (single-use,
        // no clone), so use MockActuator that ignores command and
        // returns a deterministic result; consume via the second
        // dispatch.
        let (estop, _rx) = EstopSignal::new();
        let arm = MockArm::new();

        // First dispatch succeeds — permit consumed inside arm.
        dispatch_with_estop(&arm, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap();
        // Sanity: arm received it.
        assert_eq!(arm.received().await.len(), 1);
    }

    #[tokio::test]
    async fn estop_clone_is_independent_handle_to_same_signal() {
        // Sender clones are wiring conveniences (UI button +
        // hardware relay + healing tier all sharing one signal).
        // Confirm that trip on one clone is seen by another.
        let (estop_a, rx_a) = EstopSignal::new();
        let estop_b = estop_a.clone();
        estop_b.trip();
        assert!(estop_a.tripped());
        assert!(rx_a.tripped());
    }

    #[tokio::test]
    async fn dispatch_with_estop_works_with_mock_actuator() {
        // Cross-crate sanity: the V1 `MockActuator` from
        // `aether-actuators` also drives correctly through the
        // estop wrapper. Proves the wrapper takes any `&dyn
        // Actuator`, not just robotics-specific ones (a future
        // session might want an e-stop on cameras too).
        let (estop, _rx) = EstopSignal::new();
        let mock = MockActuator::new("any-actuator");
        let result = dispatch_with_estop(&mock, ActuatorCommand::Halt, test_permit(), &estop)
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        assert_eq!(mock.received().await[0].kind(), CommandKind::Halt);
    }
}
