//! Industrial protocol abstraction.
//!
//! Domain code (work orders, telemetry ingest) talks to a `Bridge` trait.
//! Concrete impls live in `aether-opcua`, `aether-mqtt`, and (future)
//! Modbus/Profinet/EtherNet-IP crates. Wiring a new protocol means
//! implementing the trait and registering the bridge in `TagRegistry`.
//!
//! See `docs/architecture/protocols.md`.

pub mod bridge;
pub mod registry;
pub mod sample;

pub use bridge::{Bridge, BridgeError, BridgeId, BridgeKind, BridgeStatus};
pub use registry::{Binding, TagRegistry, TagRegistryError};
pub use sample::{Quality, TagSample};
