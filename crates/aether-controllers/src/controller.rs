//! `Controller` — PLC / PAC / CNC / DCS as an
//! [`aether_actuators::Actuator`].
//!
//! ## Trait shape
//! - Writes go through the V1 `Actuator::dispatch` path with a
//!   `ActuatorCommand::WriteTag { address, value }` payload. The
//!   permit gate + e-stop preemption (V8) come for free.
//! - Reads use a dedicated `read_tag` method on this trait —
//!   side-effect-free, no permit needed (see crate-level
//!   docstring for the rationale).
//! - `controller_id()` returns the typed `ControllerId` so cross-
//!   table joins on `tag_writes.controller_id` use the right
//!   newtype. `controller_kind()` returns the `ControllerKind`
//!   discriminator for routing.

use crate::kind::ControllerKind;
use crate::tag::TagAddress;
use aether_actuators::actuator::Actuator;
use aether_actuators::command::TagValue;
use aether_core::ControllerId;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReadError {
    /// Tag isn't declared on this controller. Hands-off-the-PLC
    /// principle: we don't silently read whatever address the
    /// caller asks for — the controller knows its declared tags
    /// and surfaces unknown reads as a typed error so the caller
    /// can fix the binding rather than reading garbage.
    #[error("unknown tag: {0}")]
    UnknownTag(String),
    /// Transport-level failure (network, serial, USB). Usually
    /// retryable.
    #[error("transport: {0}")]
    Transport(String),
    /// Tag exists but its current value couldn't be coerced to
    /// the requested type. Indicates a binding mismatch (e.g.
    /// caller asked for Bool from a Float register).
    #[error("type mismatch: tag {0} has wrong type for requested read")]
    TypeMismatch(String),
}

#[derive(Debug, Error)]
pub enum WriteError {
    /// Tag isn't declared on this controller — same rationale as
    /// `ReadError::UnknownTag`.
    #[error("unknown tag: {0}")]
    UnknownTag(String),
    /// Value type doesn't match the tag's declared type
    /// (e.g. attempting `TagValue::Float` against a Bool tag).
    /// Surface explicitly so the caller fixes the typing rather
    /// than relying on silent vendor coercion.
    #[error("type mismatch: tag {tag} declared as {declared}, write attempted as {attempted}")]
    TypeMismatch {
        tag: String,
        declared: &'static str,
        attempted: &'static str,
    },
    /// Transport-level failure. Usually retryable.
    #[error("transport: {0}")]
    Transport(String),
    /// Vendor-level rejection (out-of-range, read-only tag,
    /// safety-locked). Not retryable without changing inputs.
    #[error("vendor refused write: {0}")]
    VendorRefused(String),
}

#[async_trait]
pub trait Controller: Actuator {
    /// Stable typed identity. Distinct from the underlying
    /// `ActuatorId` superset so cross-table joins use the right
    /// projection.
    fn controller_id(&self) -> ControllerId;

    /// Discriminator across the four controller families. Used by
    /// the binding wizard to surface vendor-specific options
    /// (e.g. "load NC program" for Cnc only).
    fn controller_kind(&self) -> ControllerKind;

    /// Read a single tag value. Side-effect-free; bypasses the V1
    /// permit gate (see crate docstring). Returns the current
    /// value as a `TagValue` carrying its IEC-61131 type tag.
    ///
    /// Implementations MUST return `Err(ReadError::UnknownTag)`
    /// for addresses they haven't declared rather than silently
    /// returning a zero value — silent zero would mask binding
    /// bugs that cause the UI to display "OK" when there's no
    /// data on the wire.
    async fn read_tag(&self, address: &TagAddress) -> Result<TagValue, ReadError>;
}
