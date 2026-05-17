//! IIoT gateways and edge servers as bidirectional
//! [`aether_actuators::Actuator`]s with locally-buffered
//! telemetry store-and-forward.
//!
//! ## What V15 ships (this crate's first commit, final Phase 2)
//!
//!   * [`kind::GatewayKind`] — discriminator across the three
//!     gateway classes that show up on industrial networks:
//!       - **Industrial** — rugged DIN-rail gateways with serial
//!         and Ethernet uplinks (Moxa UC-8100, Advantech UNO,
//!         eWON Cosy). Aggregate legacy RS-485 / 4–20 mA devices
//!         into IP.
//!       - **Edge** — compute-capable edge nodes (Siemens
//!         Industrial Edge, AWS IoT Greengrass, Azure IoT Edge).
//!         Run pre-processing / ML inference before forwarding.
//!       - **ProtocolBridge** — pure transport translators
//!         (Modbus → MQTT, OPC-UA → REST). No local compute,
//!         minimal buffering.
//!
//!   * [`sample::BufferedSample`] — one buffered telemetry
//!     record waiting to be forwarded upstream. Carries source
//!     topic + payload bytes + buffer timestamp so the V10 sync
//!     layer can reconstruct ordering even after a multi-hour
//!     WAN outage.
//!
//!   * [`policy::BufferPolicy`] — how the gateway handles the
//!     "buffer full" + "WAN restored" cases:
//!       - `PassThrough` — no local buffering, drop on WAN
//!         outage (acceptable for cosmetic readouts).
//!       - `BufferUntilOnline` — accumulate until WAN restored.
//!         Default for most pilots; capped by `max_buffer`.
//!       - `DropOldestOnFull` — bounded FIFO, oldest sample
//!         evicted when buffer fills. Right policy for telemetry
//!         where "the latest reading wins" semantics matter.
//!
//!   * [`gateway::Gateway`] — `Actuator` supertrait. Adds
//!     `gateway_id()`, `gateway_kind()`, `pending_samples()`,
//!     `buffer_policy()`. No new `ActuatorCommand` variant
//!     this session — gateways inherit `Halt` from V1 and don't
//!     have category-specific outbound commands (their unique
//!     value-add is the read-side buffer drain).
//!
//!   * [`mock::MockGateway`] — in-memory buffer with policy-
//!     respecting enqueue. Capacity bounded; eviction follows
//!     the configured `BufferPolicy`.
//!
//! ## Why no new ActuatorCommand variant
//! Every Phase-2 category before this (Scanner, Controller,
//! HMI) added a category-specific command (Scan, WriteTag,
//! Announce). Gateways don't naturally have an outbound
//! command shape that's distinct from the existing universe —
//! a gateway forwards data; it doesn't take orders that look
//! like "do this physical thing." Halt remains the only
//! universal command, and that's enough.
//!
//! Future sessions may add `RunEdgeFunction { function_id,
//! input }` for invoking deployed edge compute, or
//! `ForwardSample { topic, payload }` for operator-requested
//! republish. Neither is load-bearing for V15.
//!
//! ## Trust boundary
//! The gateway buffers but doesn't authenticate samples.
//! Upstream consumers MUST verify provenance (vendor / device /
//! signature) before acting — the gateway is the transport
//! fabric, not the trust root. A future session can layer a
//! `SampleAttestation` policy on top without changing this
//! trait surface.

pub mod gateway;
pub mod kind;
pub mod mock;
pub mod policy;
pub mod sample;

pub use gateway::Gateway;
pub use kind::GatewayKind;
pub use mock::{BufferError, MockGateway};
pub use policy::{BufferPolicy, DEFAULT_BUFFER_CAPACITY};
pub use sample::BufferedSample;
