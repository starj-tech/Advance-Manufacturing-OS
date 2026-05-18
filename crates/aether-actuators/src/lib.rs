//! Bidirectional hardware actuator framework — `aether-vision` and
//! `aether-robotics` both consume the trait + permit gating defined
//! here.
//!
//! ## Why a separate crate
//! Vision (cameras, defect detectors) and Robotics (arms, AGVs/AMRs)
//! both need to send commands to physical hardware AND both need the
//! same safety guarantees: every command must pass the safety
//! interlock, must be idempotent, must be cancellable on emergency
//! stop. Centralizing those guarantees here means:
//!
//!   * One place enforces "no dispatch without interlock". The
//!     [`Actuator::dispatch`] signature takes a [`permit::ActuatorPermit`]
//!     whose only constructor lives behind [`gate::gate`] — which itself
//!     calls into `aether_safety::interlock::evaluate`. The type system
//!     rejects code paths that skip the check.
//!   * One place defines the [`command::ActuatorCommand`] enum, so a
//!     new command variant (e.g. `EmergencyHalt`) is a one-line
//!     extension that every Actuator impl must match exhaustively.
//!   * One place owns permit lifecycle (TTL, single-use, e-stop
//!     pre-emption). New Actuator impls get all of that for free.
//!
//! ## What this crate is NOT
//! It is NOT a transport. Each Actuator implementation chooses its
//! own transport: HTTP REST for cameras, OPC-UA for PLC-driven arms,
//! ROS-bridge for ROS-native robots, etc. The Actuator trait sits
//! above transport — its job is structure, safety, and audit, not
//! bytes-on-the-wire.
//!
//! ## Failure semantics
//! Every error path the Actuator trait can return is enumerated in
//! [`actuator::ActuatorError`] so callers can branch on the failure
//! class:
//!   * `PermitConsumed` — permit was already used; need a fresh `gate()`
//!   * `PermitExpired` — permit's TTL elapsed before dispatch
//!   * `Estopped` — emergency stop active; no commands accepted
//!   * `InterlockDenied(Verdict)` — final pre-dispatch check failed
//!     (e.g. cert revoked between gate and dispatch)
//!   * `Transport(String)` — protocol/network failure; usually retryable
//!   * `BadCommand(String)` — command violates a hardware invariant
//!     (e.g. joint angle out of range)

pub mod actuator;
pub mod command;
pub mod gate;
pub mod permit;
pub mod traced;

pub use actuator::{Actuator, ActuatorError, ActuatorResult, MockActuator};
pub use command::{ActuatorCommand, AnnounceSeverity, CommandKind, ScanTrigger, TagValue};
pub use gate::{gate, GateError};
pub use permit::{ActuatorPermit, PermitTtl, DEFAULT_PERMIT_TTL};
pub use traced::{outcome_slug, traced_dispatch};
