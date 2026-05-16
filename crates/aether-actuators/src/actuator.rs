//! The `Actuator` trait — every controllable hardware device implements
//! it, and `dispatch` is the one entry point through which commands
//! cross from software into hardware.
//!
//! ## Why `dispatch` takes the permit by value
//! Single-use semantics: once `dispatch` is called, the permit is gone
//! (moved into the function). Callers that pass the same permit twice
//! get a compile error, not a runtime "already consumed" failure.
//! Inside `dispatch`, the implementation calls
//! [`crate::ActuatorPermit::try_consume`] to atomically mark the permit
//! spent — so even if the value were cloned (it implements `Clone` for
//! API-symmetry reasons), the second consume attempt fails at runtime.
//!
//! ## Why dispatch is async
//! Real hardware transport is network/serial/USB I/O. All known
//! implementations will await network or device round-trips; making
//! the trait sync would force every impl into `block_on` or thread
//! pools. Async-trait via `#[async_trait]` is the same pattern
//! `Healer`, `Probe`, `Bridge`, and `CommodityFeed` already use.
//!
//! ## Why a "kind" hint on the trait
//! The dispatcher and ledger need to log "what kind of device was
//! this command sent to" without holding a reference to the concrete
//! type. [`Actuator::actuator_kind`] returns a short slug
//! (`"camera"`, `"robot-arm"`, `"agv"`) for that purpose.

use crate::command::ActuatorCommand;
use crate::permit::ActuatorPermit;
use aether_core::ActuatorId;
use aether_safety::interlock::Verdict;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ActuatorError {
    /// The permit was already used by a previous `dispatch` call.
    /// Operator double-tap is the canonical trigger; retry with a
    /// fresh `gate()`.
    #[error("permit already consumed")]
    PermitConsumed,
    /// The permit's TTL elapsed before this dispatch ran. Re-gate.
    #[error("permit expired at {0}")]
    PermitExpired(DateTime<Utc>),
    /// Emergency stop is active. No commands accepted until reset.
    #[error("emergency stop active")]
    Estopped,
    /// Final pre-dispatch interlock recheck failed (e.g. cert revoked
    /// between gate and dispatch). The verdict carries detail.
    #[error("interlock denied at dispatch time: {0:?}")]
    InterlockDenied(Verdict),
    /// Network / serial / device transport failure. Usually retryable.
    #[error("transport: {0}")]
    Transport(String),
    /// Command violates a hardware invariant (joint angle out of
    /// range, unsupported format, route not found). Not retryable
    /// without changing the command.
    #[error("bad command: {0}")]
    BadCommand(String),
}

/// Successful dispatch result. Implementations return additional
/// per-command data (captured frame id, joint-actual-after-move,
/// route ack token) via `data` — the dispatcher logs it verbatim
/// into the `actuator_commands` row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActuatorResult {
    pub actuator_id: ActuatorId,
    pub command_kind: String,
    pub completed_at: DateTime<Utc>,
    pub data: serde_json::Value,
}

#[async_trait]
pub trait Actuator: Send + Sync {
    /// Stable hardware identity. Logged into every audit row.
    fn actuator_id(&self) -> ActuatorId;

    /// Short kebab-case category slug (`"camera"`, `"robot-arm"`,
    /// `"agv"`). Used for log filtering and metric labels.
    fn actuator_kind(&self) -> &'static str;

    /// Dispatch a command. The permit is moved in (single-use), and
    /// every implementation MUST:
    ///   1. Call [`ActuatorPermit::try_consume`] first; surface
    ///      [`ActuatorError::PermitConsumed`] on the loser.
    ///   2. Check permit expiry against `Utc::now()`; surface
    ///      [`ActuatorError::PermitExpired`] if elapsed.
    ///   3. Honor the [`ActuatorCommand::Halt`] variant universally,
    ///      even if the rest of the command set isn't supported.
    ///
    /// Implementations are encouraged to use the [`enforce_permit`]
    /// helper to centralize the consume + expiry checks.
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError>;
}

/// Shared helper that every `dispatch` impl SHOULD call first. Wraps
/// the consume + expiry check so each implementation doesn't re-derive
/// the order of operations (which is load-bearing — consume MUST come
/// before expiry, otherwise an expired permit could be silently
/// "wasted" without bumping the consume flag).
pub fn enforce_permit(permit: &ActuatorPermit) -> Result<(), ActuatorError> {
    if !permit.try_consume() {
        return Err(ActuatorError::PermitConsumed);
    }
    let now = Utc::now();
    if permit.is_expired_at(now) {
        return Err(ActuatorError::PermitExpired(permit.expires_at()));
    }
    Ok(())
}

// ---------- Mock for tests ----------

/// In-memory Actuator that records every command it receives. Used by
/// downstream crates (`aether-vision`, `aether-robotics`) to test
/// their pipelines without standing up real hardware.
///
/// `accept_kinds` filters: commands whose kind isn't in the list
/// surface as `BadCommand`. Default = accept all. `force_error`
/// (for the next call) lets a test scenario inject a transport
/// failure.
pub struct MockActuator {
    id: ActuatorId,
    kind: &'static str,
    accept_kinds: Vec<crate::command::CommandKind>,
    received: tokio::sync::Mutex<Vec<ActuatorCommand>>,
    force_error: tokio::sync::Mutex<Option<ActuatorError>>,
}

impl MockActuator {
    pub fn new(kind: &'static str) -> Self {
        Self {
            id: ActuatorId::new(),
            kind,
            accept_kinds: vec![],
            received: tokio::sync::Mutex::new(Vec::new()),
            force_error: tokio::sync::Mutex::new(None),
        }
    }

    /// Only accept the listed command kinds; reject others with
    /// `BadCommand`. Empty list = accept all (test default).
    pub fn accepting(mut self, kinds: Vec<crate::command::CommandKind>) -> Self {
        self.accept_kinds = kinds;
        self
    }

    /// Snapshot the commands received so far.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }

    /// Pre-queue a forced error for the next `dispatch` call. Used to
    /// exercise transport-error handling without standing up a broken
    /// network.
    pub async fn fail_next(&self, err: ActuatorError) {
        *self.force_error.lock().await = Some(err);
    }
}

/// Wrapping in Arc so the mock can be shared across pipeline tasks
/// without the trait object's auto-Send concerns.
pub type MockActuatorRef = Arc<MockActuator>;

#[async_trait]
impl Actuator for MockActuator {
    fn actuator_id(&self) -> ActuatorId {
        self.id
    }

    fn actuator_kind(&self) -> &'static str {
        self.kind
    }

    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;

        // Drain a queued forced error before doing anything else, so
        // tests can inject failures even on commands that would
        // otherwise succeed.
        if let Some(err) = self.force_error.lock().await.take() {
            return Err(err);
        }

        if !self.accept_kinds.is_empty() && !self.accept_kinds.contains(&cmd.kind()) {
            return Err(ActuatorError::BadCommand(format!(
                "{} does not accept command kind {}",
                self.kind,
                cmd.kind().slug()
            )));
        }

        let kind_slug = cmd.kind().slug().to_string();
        self.received.lock().await.push(cmd);

        Ok(ActuatorResult {
            actuator_id: self.id,
            command_kind: kind_slug,
            completed_at: Utc::now(),
            data: serde_json::json!({"mock": true}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permit::{ActuatorPermit, PermitTtl, DEFAULT_PERMIT_TTL};

    fn mock() -> MockActuator {
        MockActuator::new("test-device")
    }

    #[tokio::test]
    async fn dispatch_with_valid_permit_records_command() {
        let m = mock();
        let permit = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let cmd = ActuatorCommand::Capture {
            quality: 75,
            format: "png".into(),
        };
        let result = m.dispatch(cmd.clone(), permit).await.unwrap();
        assert_eq!(result.command_kind, "capture");
        assert_eq!(m.received().await, vec![cmd]);
    }

    #[tokio::test]
    async fn dispatch_consumes_the_permit() {
        // Single-use property exercised end-to-end through the trait.
        let m = mock();
        let permit = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let cmd = ActuatorCommand::Halt;
        m.dispatch(cmd.clone(), permit.clone()).await.unwrap();
        let err = m.dispatch(cmd, permit).await.unwrap_err();
        assert!(matches!(err, ActuatorError::PermitConsumed));
    }

    #[tokio::test]
    async fn dispatch_rejects_expired_permit() {
        let m = mock();
        let permit = ActuatorPermit::new_unchecked(PermitTtl(50));
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        let err = m.dispatch(ActuatorCommand::Halt, permit).await.unwrap_err();
        assert!(matches!(err, ActuatorError::PermitExpired(_)));
    }

    #[tokio::test]
    async fn accepting_filter_rejects_unsupported_kinds() {
        // A camera-only mock receiving a robot move — must reject
        // with BadCommand, not silently swallow.
        let m = mock().accepting(vec![crate::command::CommandKind::Capture]);
        let permit = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let err = m
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 1.0,
                },
                permit,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn fail_next_injects_transport_error_then_clears() {
        let m = mock();
        m.fail_next(ActuatorError::Transport("simulated".into()))
            .await;

        let permit = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let err = m.dispatch(ActuatorCommand::Halt, permit).await.unwrap_err();
        assert!(matches!(err, ActuatorError::Transport(_)));

        // Force-error queue is one-shot — next dispatch succeeds.
        let permit2 = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let result = m.dispatch(ActuatorCommand::Halt, permit2).await.unwrap();
        assert_eq!(result.command_kind, "halt");
    }

    #[tokio::test]
    async fn forced_error_path_still_consumes_the_permit() {
        // Critical: even on a transport failure, the permit is spent.
        // Otherwise an operator could retry the same permit endlessly,
        // which defeats the single-use safety guarantee.
        let m = mock();
        m.fail_next(ActuatorError::Transport("nope".into())).await;
        let permit = ActuatorPermit::new_unchecked(DEFAULT_PERMIT_TTL);
        let _err = m.dispatch(ActuatorCommand::Halt, permit.clone()).await;
        assert!(permit.is_consumed());
    }

    #[tokio::test]
    async fn enforce_permit_order_is_consume_then_expiry() {
        // Documenting the load-bearing order: an already-consumed
        // permit returns PermitConsumed even if it's also expired.
        // (We rely on the consume being the more informative signal —
        // tells us "operator double-tap" rather than "operator was
        // too slow".)
        let permit = ActuatorPermit::new_unchecked(PermitTtl(50));
        assert!(permit.try_consume());
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        let err = enforce_permit(&permit).unwrap_err();
        assert!(matches!(err, ActuatorError::PermitConsumed));
    }

    #[tokio::test]
    async fn full_chain_gate_then_dispatch_succeeds() {
        // End-to-end happy path through the public API: gate() →
        // permit → dispatch → ActuatorResult. The only path external
        // code is supposed to use.
        use crate::gate::gate;
        use aether_safety::interlock::{Certification, UnlockRequest};
        use chrono::Duration as ChronoDuration;
        use uuid::Uuid;

        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "robot-op".into(),
                issued_at: Utc::now() - ChronoDuration::days(30),
                expires_at: Some(Utc::now() + ChronoDuration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["robot-op".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        let permit = gate(&req).unwrap();
        let m = mock();
        let result = m
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 1,
                    target_rad: 0.5,
                },
                permit,
            )
            .await
            .unwrap();
        assert_eq!(result.command_kind, "move-joint");
        assert_eq!(result.actuator_id, m.actuator_id());
    }
}
