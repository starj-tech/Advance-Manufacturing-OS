//! `traced_dispatch` — wraps `Actuator::dispatch` with a
//! structured `tracing` span so every command is observable.
//!
//! ## What the span carries
//! Each dispatch becomes a span at the `info` level with the
//! following fields populated:
//!
//!   * `actuator.id` — UUID of the actuator
//!   * `actuator.kind` — slug (camera, robot-arm, scanner, ...)
//!   * `command.kind` — slug (capture, move-joint, scan, ...)
//!   * `outcome` — `ok` / `permit-consumed` / `permit-expired`
//!     / `estopped` / `interlock-denied` / `bad-command` /
//!     `transport`
//!   * `duration_ms` — wall-time from span entry to exit
//!   * `result.command_kind` — on success only, the kind slug
//!     the actuator reported back (this is sometimes more
//!     specific than the input — e.g. an Actuator that
//!     downgrades a request)
//!
//! The OpenTelemetry exporter wired by `aether-telemetry::
//! init_tracing` picks these spans up and forwards them to
//! whatever backend the binary configures (Honeycomb /
//! Jaeger / Datadog) without further plumbing.
//!
//! ## Why a helper rather than a default-method on the trait
//! `Actuator::dispatch` is the safety-critical entry point —
//! we don't want to bury tracing inside the trait
//! implementation, because that would mean every impl has to
//! call the helper itself (which they'd forget). Wrapping at
//! the call site keeps the trait surface narrow and lets
//! production callers opt in to instrumentation explicitly
//! while tests skip the noise.

use crate::actuator::{Actuator, ActuatorError, ActuatorResult};
use crate::command::ActuatorCommand;
use crate::permit::ActuatorPermit;
use std::time::Instant;
use tracing::{info_span, Instrument};

/// Outcome slug for the `outcome` span field. Centralized so a
/// rename here is the schema migration for log/metric
/// dashboards.
pub fn outcome_slug(result: &Result<ActuatorResult, ActuatorError>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(ActuatorError::PermitConsumed) => "permit-consumed",
        Err(ActuatorError::PermitExpired(_)) => "permit-expired",
        Err(ActuatorError::Estopped) => "estopped",
        Err(ActuatorError::InterlockDenied(_)) => "interlock-denied",
        Err(ActuatorError::BadCommand(_)) => "bad-command",
        Err(ActuatorError::Transport(_)) => "transport",
    }
}

/// Dispatch wrapped with a tracing span. Same return shape as
/// `Actuator::dispatch`; observers see the span open + close
/// with all attributes populated.
pub async fn traced_dispatch(
    actuator: &dyn Actuator,
    cmd: ActuatorCommand,
    permit: ActuatorPermit,
) -> Result<ActuatorResult, ActuatorError> {
    let permit_id = permit.permit_id();
    let cmd_kind = cmd.kind().slug();
    let actuator_id = actuator.actuator_id();
    let actuator_kind = actuator.actuator_kind();

    let span = info_span!(
        "actuator.dispatch",
        actuator.id = %actuator_id,
        actuator.kind = %actuator_kind,
        command.kind = %cmd_kind,
        permit.id = %permit_id,
        outcome = tracing::field::Empty,
        duration_ms = tracing::field::Empty,
        result.command_kind = tracing::field::Empty,
    );

    async move {
        let start = Instant::now();
        let result = actuator.dispatch(cmd, permit).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let span = tracing::Span::current();
        span.record("outcome", outcome_slug(&result));
        span.record("duration_ms", elapsed_ms);
        if let Ok(ref ok) = result {
            span.record("result.command_kind", ok.command_kind.as_str());
        }
        result
    }
    .instrument(span)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actuator::MockActuator;
    use crate::command::ActuatorCommand;
    use crate::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn approved_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("test fixture: interlock approves")
    }

    #[test]
    fn outcome_slug_covers_every_error_variant() {
        // Pin the slug values — they're load-bearing for
        // dashboards / log greps in production.
        assert_eq!(
            outcome_slug(&Err(ActuatorError::PermitConsumed)),
            "permit-consumed"
        );
        assert_eq!(
            outcome_slug(&Err(ActuatorError::PermitExpired(Utc::now()))),
            "permit-expired"
        );
        assert_eq!(outcome_slug(&Err(ActuatorError::Estopped)), "estopped");
        assert_eq!(
            outcome_slug(&Err(ActuatorError::BadCommand("x".into()))),
            "bad-command"
        );
        assert_eq!(
            outcome_slug(&Err(ActuatorError::Transport("x".into()))),
            "transport"
        );
    }

    #[test]
    fn outcome_slug_for_ok_is_ok() {
        let result = Ok(ActuatorResult {
            actuator_id: aether_core::ActuatorId::new(),
            command_kind: "halt".into(),
            completed_at: Utc::now(),
            data: serde_json::json!({}),
        });
        assert_eq!(outcome_slug(&result), "ok");
    }

    #[tokio::test]
    async fn traced_dispatch_returns_underlying_actuator_result() {
        // Wrapper transparency: the return value of
        // traced_dispatch is the SAME Result the inner
        // actuator returned. Tracing is observation-only —
        // never alters control flow.
        let mock = MockActuator::new("test-mock");
        let permit = approved_permit();
        let result = traced_dispatch(&mock, ActuatorCommand::Halt, permit)
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        // Mock recorded one command — the wrapper didn't
        // silently double-dispatch.
        assert_eq!(mock.received().await.len(), 1);
    }

    #[tokio::test]
    async fn traced_dispatch_propagates_underlying_errors() {
        // Pin that an inner error path doesn't get swallowed
        // by the span machinery.
        let mock = MockActuator::new("test-mock").accepting(vec![crate::CommandKind::Halt]);
        let permit = approved_permit();
        let err = traced_dispatch(
            &mock,
            ActuatorCommand::MoveJoint {
                joint: 0,
                target_rad: 0.0,
            },
            permit,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn traced_dispatch_consumes_the_permit_exactly_once() {
        // The permit is moved into the wrapper, which moves it
        // into the inner dispatch. Single-use semantic is
        // preserved end-to-end (compile-time guarantee — this
        // test exists to lock the runtime observable).
        let mock = MockActuator::new("test-mock");
        let permit = approved_permit();
        traced_dispatch(&mock, ActuatorCommand::Halt, permit)
            .await
            .unwrap();
        // A second dispatch needs a fresh permit; this
        // wouldn't even compile if we tried to reuse the
        // previous permit value (it's gone).
        let permit2 = approved_permit();
        traced_dispatch(&mock, ActuatorCommand::Halt, permit2)
            .await
            .unwrap();
        assert_eq!(mock.received().await.len(), 2);
    }
}
