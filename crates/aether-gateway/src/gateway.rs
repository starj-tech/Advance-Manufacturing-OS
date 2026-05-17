//! `Gateway` — IIoT gateway / edge server / protocol bridge as
//! an [`aether_actuators::Actuator`].
//!
//! ## Trait shape
//! Reads dominate: `pending_samples` drains the local buffer
//! FIFO (drain semantic mirrors V14's HMI `pending_events` —
//! each sample delivered exactly once across drains, no
//! double-counting in the upstream sync ledger).
//!
//! No category-specific outbound command. The gateway inherits
//! `Halt` from V1; that's the only command shape that makes
//! sense at the trait surface today. A future session may add
//! `RunEdgeFunction { id, input }` for invoking deployed
//! compute on `Edge`-class gateways, but that needs an edge
//! function registry that's out of scope for V15.
//!
//! ## Why `buffer_policy` is on the trait, not just config
//! Downstream consumers (sync layer, healing layer) branch on
//! the policy to decide their own behavior — a gateway with
//! `BufferUntilOnline` warrants a heal-fault alert when its
//! buffer fills; a `DropOldestOnFull` does not. Exposing the
//! policy through the trait keeps that branching honest
//! (callers can't accidentally hold stale config).

use crate::kind::GatewayKind;
use crate::policy::BufferPolicy;
use crate::sample::BufferedSample;
use aether_actuators::actuator::Actuator;
use aether_core::GatewayId;
use async_trait::async_trait;

#[async_trait]
pub trait Gateway: Actuator {
    /// Stable typed identity. Distinct from `ActuatorId` so
    /// cross-table joins on `buffered_samples.gateway_id` use
    /// the right newtype projection.
    fn gateway_id(&self) -> GatewayId;

    /// Discriminator across the three gateway classes
    /// (Industrial / Edge / ProtocolBridge). Drives sync
    /// cadence + healing-layer routing.
    fn gateway_kind(&self) -> GatewayKind;

    /// Effective buffer policy. Pinned at construction in the
    /// mock; production impls expose the live config so the
    /// healing layer can react to runtime changes.
    fn buffer_policy(&self) -> BufferPolicy;

    /// Drain the locally-buffered samples. Returns them in
    /// FIFO order (oldest first); impls MUST clear the buffer
    /// after returning so a second drain doesn't double-count
    /// in the upstream sync ledger.
    ///
    /// Returns `Ok(vec![])` for "buffer empty" — callers should
    /// NOT treat this as an error. WAN-up flows poll this on
    /// a cadence to forward batches upstream.
    async fn pending_samples(&self) -> Vec<BufferedSample>;
}
