//! Industrial controllers — PLC / PAC / CNC / DCS as
//! bidirectional [`aether_actuators::Actuator`]s with typed tag
//! read/write.
//!
//! ## What V13 ships (this crate's first commit)
//!
//!   * [`kind::ControllerKind`] — discriminator across the four
//!     controller families that show up in factories:
//!       - **PLC** — programmable logic controller, the workhorse
//!         of discrete manufacturing (Siemens S7, Allen-Bradley
//!         ControlLogix, Beckhoff TwinCAT).
//!       - **PAC** — programmable automation controller, the
//!         richer cousin with built-in motion / vision /
//!         networking (Rockwell CompactLogix, Schneider M580).
//!       - **CNC** — numerical control for machine tools
//!         (FANUC, Siemens Sinumerik, Heidenhain).
//!       - **DCS** — distributed control system for process
//!         plants (Emerson DeltaV, Honeywell Experion).
//!
//!   * [`controller::Controller`] — `Actuator` supertrait. Adds
//!     `controller_id()`, `controller_kind()`, and a pure-read
//!     `read_tag(address)` method that bypasses the V1 permit
//!     gate. Writes go through `ActuatorCommand::WriteTag`
//!     (added to the workspace enum this session) so every
//!     state-changing tag write is permit-gated like every other
//!     Actuator command.
//!
//!   * [`tag::TagAddress`] — typed wrapper around the vendor-
//!     defined symbol string. Doesn't parse — vendors disagree on
//!     syntax and the controller impl is the only layer that
//!     needs to understand them — but pins the type so a misuse
//!     ("I passed a `String` as the value") is a compile error.
//!
//!   * [`mock::MockController`] — in-memory tag map for tests +
//!     downstream pipeline development. Supports register-time
//!     declaration of tags + their typed value, then reads /
//!     writes against the map with strict type checking.
//!
//! ## Why reads bypass the gate
//! Reading a tag is side-effect-free at the hardware level — the
//! PLC processor doesn't change state when a SCADA client polls a
//! coil. Subjecting reads to the V1 permit gate would force every
//! UI polling loop through `gate()`, which would dominate the
//! interlock-events table without adding safety. Writes are the
//! state-changing operation; they go through `WriteTag` and the
//! permit-gated dispatch path.
//!
//! That said: reading a PRIVILEGED tag (recipe, safety config)
//! may need a separate authorization layer. That's intentionally
//! out of scope for V13 — a future session can add a
//! `ReadPolicy` trait that decides per-tag whether to require a
//! permit, without changing the trait surface here.
//!
//! ## What lands later
//!   * V14 — CNC-specific commands: `LoadProgram { ncc_id }`,
//!     `ToolChange { tool_id }`, `SpindleOverride { factor }`.
//!     These layer on top of the V13 Controller trait without
//!     changing it.
//!   * V15+ — real OPC-UA / Modbus impls behind the trait once
//!     the V1 protocol crate impls land.

pub mod controller;
pub mod kind;
pub mod mock;
pub mod tag;

pub use controller::{Controller, ReadError, WriteError};
pub use kind::ControllerKind;
pub use mock::{MockController, TagDeclaration, TagDeclarationError};
pub use tag::{TagAddress, TagAddressError};
