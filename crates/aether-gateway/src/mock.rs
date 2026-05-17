//! [`MockGateway`] — in-memory buffer with policy-respecting
//! enqueue.
//!
//! ## Policy semantics under the hood
//! - `PassThrough` — enqueue is a no-op; `pending_samples`
//!   always returns empty. Test fixtures that try to inject
//!   samples get `BufferError::PolicyDisallowsBuffering` so
//!   the misuse surfaces loudly.
//! - `BufferUntilOnline` — bounded VecDeque; enqueue at cap
//!   surfaces `BufferError::Full`. The operator can drain via
//!   `pending_samples` (mirrors "WAN restored, forward the
//!   backlog") to free space.
//! - `DropOldestOnFull` — bounded VecDeque; enqueue at cap
//!   evicts the front and pushes the new entry. The eviction
//!   count is tracked so audit can answer "how many samples
//!   did we drop during this outage."
//!
//! ## Audit-uniformity
//! All commands received by dispatch (Halt and the rejected
//! variants) appear in the `received` log so investigators
//! see what was attempted. Same pattern as MockCamera,
//! MockArm, MockAgv, MockScanner, MockController, MockHmi.

use crate::gateway::Gateway;
use crate::kind::GatewayKind;
use crate::policy::{BufferPolicy, DEFAULT_BUFFER_CAPACITY};
use crate::sample::{BufferedSample, MAX_PAYLOAD_BYTES};
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, GatewayId};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum BufferError {
    /// Buffer is full and the policy is `BufferUntilOnline` (no
    /// eviction). Operator's right move is to investigate why
    /// the WAN is down or to drain the buffer manually.
    #[error("buffer full at capacity {0} (policy: buffer-until-online)")]
    Full(usize),
    /// Tried to enqueue against a `PassThrough` gateway — the
    /// policy explicitly forbids buffering. Catches the test-
    /// fixture bug of feeding samples to a non-buffering
    /// gateway and silently losing them.
    #[error("gateway policy is pass-through; buffering not allowed")]
    PolicyDisallowsBuffering,
    /// Sample payload exceeds the `MAX_PAYLOAD_BYTES` cap.
    /// Large blobs (video frames, ML models) go through the
    /// V4 sealed-frame path instead of the telemetry buffer.
    #[error("payload {actual} bytes exceeds cap {cap}")]
    PayloadTooLarge { actual: usize, cap: usize },
}

pub struct MockGateway {
    actuator_id: ActuatorId,
    gateway_id: GatewayId,
    kind: GatewayKind,
    policy: BufferPolicy,
    capacity: usize,
    buffer: Mutex<VecDeque<BufferedSample>>,
    evicted: AtomicUsize,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockGateway {
    pub fn new(kind: GatewayKind, policy: BufferPolicy) -> Self {
        Self::with_capacity(kind, policy, DEFAULT_BUFFER_CAPACITY)
    }

    pub fn with_capacity(kind: GatewayKind, policy: BufferPolicy, capacity: usize) -> Self {
        Self {
            actuator_id: ActuatorId::new(),
            gateway_id: GatewayId::new(),
            kind,
            policy,
            capacity,
            buffer: Mutex::new(VecDeque::new()),
            evicted: AtomicUsize::new(0),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Enqueue a sample, respecting the configured policy.
    /// Returns `BufferError::Full` for `BufferUntilOnline` at
    /// cap; silently evicts for `DropOldestOnFull`; refuses
    /// outright for `PassThrough`.
    pub async fn enqueue_sample(&self, sample: BufferedSample) -> Result<(), BufferError> {
        if sample.payload.len() > MAX_PAYLOAD_BYTES {
            return Err(BufferError::PayloadTooLarge {
                actual: sample.payload.len(),
                cap: MAX_PAYLOAD_BYTES,
            });
        }
        match self.policy {
            BufferPolicy::PassThrough => Err(BufferError::PolicyDisallowsBuffering),
            BufferPolicy::BufferUntilOnline => {
                let mut buf = self.buffer.lock().await;
                if buf.len() >= self.capacity {
                    return Err(BufferError::Full(self.capacity));
                }
                buf.push_back(sample);
                Ok(())
            }
            BufferPolicy::DropOldestOnFull => {
                let mut buf = self.buffer.lock().await;
                if buf.len() >= self.capacity {
                    buf.pop_front();
                    self.evicted.fetch_add(1, Ordering::SeqCst);
                }
                buf.push_back(sample);
                Ok(())
            }
        }
    }

    /// Cumulative eviction count under `DropOldestOnFull`.
    /// Tests assert this matches expected drop counts.
    pub fn evicted_count(&self) -> usize {
        self.evicted.load(Ordering::SeqCst)
    }

    /// Current buffer depth. Drops to zero after each
    /// `pending_samples` drain.
    pub async fn buffer_depth(&self) -> usize {
        self.buffer.lock().await.len()
    }

    /// Effective capacity — what the policy is bounded against.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Snapshot the commands received so far (audit-uniformity
    /// pattern).
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }
}

#[async_trait]
impl Actuator for MockGateway {
    fn actuator_id(&self) -> ActuatorId {
        self.actuator_id
    }
    fn actuator_kind(&self) -> &'static str {
        "gateway"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::Halt => {
                // Halt flushes the buffer. Rationale: an e-stop
                // on the cell means the data that was buffered
                // for upstream forwarding is no longer the
                // operator's most-recent reality — after a
                // halt, the upstream consumer should see the
                // post-halt state, not stale pre-halt buffer.
                // Real industrial gateways behave similarly:
                // on cell e-stop the buffer is treated as
                // suspect and flushed by the operator's reset
                // routine.
                self.buffer.lock().await.clear();
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "gateway does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl Gateway for MockGateway {
    fn gateway_id(&self) -> GatewayId {
        self.gateway_id
    }
    fn gateway_kind(&self) -> GatewayKind {
        self.kind
    }
    fn buffer_policy(&self) -> BufferPolicy {
        self.policy
    }
    async fn pending_samples(&self) -> Vec<BufferedSample> {
        // Drain semantic — same as V14 HMI pending_events:
        // each sample delivered exactly once across drains.
        let mut buf = self.buffer.lock().await;
        buf.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_actuators::command::CommandKind;
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use uuid::Uuid;

    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "gateway-admin".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["gateway-admin".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock approves in test fixture")
    }

    fn fixture_sample(topic: &str, bytes: &[u8]) -> BufferedSample {
        BufferedSample {
            topic: topic.into(),
            payload: bytes.to_vec(),
            buffered_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn fresh_gateway_has_empty_buffer() {
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        assert_eq!(g.buffer_depth().await, 0);
        assert!(g.pending_samples().await.is_empty());
        assert_eq!(g.evicted_count(), 0);
    }

    #[tokio::test]
    async fn enqueue_then_drain_returns_samples_in_fifo_order() {
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        g.enqueue_sample(fixture_sample("a", b"1")).await.unwrap();
        g.enqueue_sample(fixture_sample("b", b"2")).await.unwrap();
        let drained = g.pending_samples().await;
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].topic, "a");
        assert_eq!(drained[1].topic, "b");
    }

    #[tokio::test]
    async fn drain_clears_buffer_so_second_call_returns_empty() {
        // Sync-ledger correctness: each sample delivered
        // exactly once across drains.
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        g.enqueue_sample(fixture_sample("a", b"1")).await.unwrap();
        assert_eq!(g.pending_samples().await.len(), 1);
        assert_eq!(g.pending_samples().await.len(), 0);
    }

    #[tokio::test]
    async fn pass_through_policy_refuses_enqueue_with_typed_error() {
        // Misuse-loud: a PassThrough gateway shouldn't silently
        // swallow samples — test fixture or production wiring
        // bug should surface immediately.
        let g = MockGateway::new(GatewayKind::ProtocolBridge, BufferPolicy::PassThrough);
        let err = g
            .enqueue_sample(fixture_sample("x", b"1"))
            .await
            .unwrap_err();
        assert!(matches!(err, BufferError::PolicyDisallowsBuffering));
        assert_eq!(g.buffer_depth().await, 0);
    }

    #[tokio::test]
    async fn buffer_until_online_at_cap_surfaces_full_error() {
        // The healing-layer signal: a full BufferUntilOnline is
        // a faulted gateway; surface as typed Full(capacity).
        let g =
            MockGateway::with_capacity(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline, 2);
        g.enqueue_sample(fixture_sample("a", b"1")).await.unwrap();
        g.enqueue_sample(fixture_sample("b", b"2")).await.unwrap();
        let err = g
            .enqueue_sample(fixture_sample("c", b"3"))
            .await
            .unwrap_err();
        assert!(matches!(err, BufferError::Full(2)));
        // Buffer didn't grow past cap; nothing evicted.
        assert_eq!(g.buffer_depth().await, 2);
        assert_eq!(g.evicted_count(), 0);
    }

    #[tokio::test]
    async fn drop_oldest_evicts_front_and_pushes_back_silently() {
        // High-frequency telemetry pattern: keep advancing,
        // record drops in evicted_count so audit can answer
        // "how much did we lose during the outage."
        let g = MockGateway::with_capacity(GatewayKind::Edge, BufferPolicy::DropOldestOnFull, 2);
        g.enqueue_sample(fixture_sample("a", b"1")).await.unwrap();
        g.enqueue_sample(fixture_sample("b", b"2")).await.unwrap();
        g.enqueue_sample(fixture_sample("c", b"3")).await.unwrap();
        assert_eq!(g.buffer_depth().await, 2);
        assert_eq!(g.evicted_count(), 1);
        // The oldest was evicted, FIFO order preserved.
        let drained = g.pending_samples().await;
        assert_eq!(drained[0].topic, "b");
        assert_eq!(drained[1].topic, "c");
    }

    #[tokio::test]
    async fn payload_at_cap_is_accepted_inclusive() {
        // Boundary: exactly MAX_PAYLOAD_BYTES is fine.
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        let sample = BufferedSample {
            topic: "x".into(),
            payload: vec![0u8; MAX_PAYLOAD_BYTES],
            buffered_at: Utc::now(),
        };
        g.enqueue_sample(sample).await.unwrap();
        assert_eq!(g.buffer_depth().await, 1);
    }

    #[tokio::test]
    async fn payload_over_cap_is_rejected_with_typed_error() {
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        let oversized = BufferedSample {
            topic: "x".into(),
            payload: vec![0u8; MAX_PAYLOAD_BYTES + 1],
            buffered_at: Utc::now(),
        };
        let err = g.enqueue_sample(oversized).await.unwrap_err();
        assert!(matches!(err, BufferError::PayloadTooLarge { .. }));
    }

    #[tokio::test]
    async fn halt_flushes_buffer() {
        // Operational invariant: post-halt buffer is suspect.
        // Mirrors real industrial-gateway recovery routines.
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        g.enqueue_sample(fixture_sample("a", b"1")).await.unwrap();
        g.enqueue_sample(fixture_sample("b", b"2")).await.unwrap();
        assert_eq!(g.buffer_depth().await, 2);
        g.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(g.buffer_depth().await, 0);
    }

    #[tokio::test]
    async fn non_halt_commands_are_rejected_with_bad_command() {
        // Gateways don't take outbound commands beyond Halt.
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        for cmd in [
            ActuatorCommand::Capture {
                quality: 80,
                format: "png".into(),
            },
            ActuatorCommand::MoveJoint {
                joint: 0,
                target_rad: 0.0,
            },
            ActuatorCommand::WriteTag {
                address: "x".into(),
                value: aether_actuators::TagValue::Bool(true),
            },
            ActuatorCommand::Announce {
                severity: aether_actuators::AnnounceSeverity::Info,
                summary: "x".into(),
                body: None,
            },
        ] {
            let err = g.dispatch(cmd, test_permit()).await.unwrap_err();
            assert!(matches!(err, ActuatorError::BadCommand(_)));
        }
    }

    #[tokio::test]
    async fn dispatched_and_rejected_commands_all_appear_in_received_log() {
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        g.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        let _ = g
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 80,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await;
        let log = g.received().await;
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].kind(), CommandKind::Halt);
        assert_eq!(log[1].kind(), CommandKind::Capture);
    }

    #[tokio::test]
    async fn gateway_kind_and_policy_returned_verbatim() {
        for (k, p) in [
            (GatewayKind::Industrial, BufferPolicy::BufferUntilOnline),
            (GatewayKind::Edge, BufferPolicy::DropOldestOnFull),
            (GatewayKind::ProtocolBridge, BufferPolicy::PassThrough),
        ] {
            let g = MockGateway::new(k, p);
            assert_eq!(g.gateway_kind(), k);
            assert_eq!(g.buffer_policy(), p);
        }
    }

    #[tokio::test]
    async fn gateway_id_distinct_from_actuator_id() {
        let g = MockGateway::new(GatewayKind::Industrial, BufferPolicy::BufferUntilOnline);
        assert_ne!(g.gateway_id().0, g.actuator_id().0);
    }

    #[tokio::test]
    async fn capacity_is_returned_verbatim_from_constructor() {
        let g = MockGateway::with_capacity(
            GatewayKind::Industrial,
            BufferPolicy::BufferUntilOnline,
            500,
        );
        assert_eq!(g.capacity(), 500);
    }
}
