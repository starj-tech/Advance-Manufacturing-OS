//! `Hmi` — HMI surface as an [`aether_actuators::Actuator`].
//!
//! ## Trait shape
//! - Outbound: announcements / prompts / alerts flow through the
//!   V1 `Actuator::dispatch` path with the
//!   `ActuatorCommand::Announce { severity, summary, body }`
//!   payload added this session. Permit gate + e-stop preemption
//!   apply uniformly.
//! - Inbound: operator interactions accumulate inside the impl
//!   and the orchestrator drains them via `pending_events`. The
//!   call returns the events accumulated since the previous
//!   drain — like reading a circular buffer, not subscribing to
//!   a stream.
//!
//! ## Why `pending_events` returns Vec instead of a stream
//! Object safety + cadence control. The orchestrator decides how
//! often to drain (60 Hz for an interactive workflow, once-per-
//! task for a long batch). A push-based stream would force every
//! impl to manage subscriber lifecycles. Pull-based matches the
//! V11 `Scanner::last_scan` pattern and stays object-safe.

use crate::event::OperatorEvent;
use crate::kind::HmiKind;
use aether_actuators::actuator::Actuator;
use aether_core::HmiId;
use async_trait::async_trait;

#[async_trait]
pub trait Hmi: Actuator {
    /// Stable typed identity. Distinct from `ActuatorId` so
    /// cross-table joins on `operator_events.hmi_id` use the
    /// right newtype projection.
    fn hmi_id(&self) -> HmiId;

    /// Surface class — drives routing decisions (don't queue a
    /// Voice prompt to a Touchscreen, don't queue a GazeDwell
    /// handler to a Pendant). The `HmiKind::supports_voice`
    /// / `supports_gaze` helpers are the canonical predicates.
    fn hmi_kind(&self) -> HmiKind;

    /// Drain the surface's accumulated operator events. Returns
    /// events in the order the surface saw them; impls MUST
    /// clear the buffer after returning (next call should NOT
    /// return the same events again — that would double-count
    /// in the audit ledger).
    ///
    /// Returns `Ok(vec![])` for "no events since last drain" —
    /// callers should NOT treat this as an error.
    async fn pending_events(&self) -> Vec<OperatorEvent>;
}
