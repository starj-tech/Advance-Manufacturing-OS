//! HMI surfaces — touchscreens, rugged tablets, smart glasses,
//! pendants — as bidirectional [`aether_actuators::Actuator`]s.
//!
//! ## What V14 ships
//!
//!   * [`kind::HmiKind`] — discriminator across the four surface
//!     classes that show up on shop floors:
//!       - **Touchscreen** — fixed-mount factory HMI (Siemens
//!         Comfort Panel, AB PanelView, Schneider Magelis).
//!       - **RuggedTablet** — Zebra ET6x / Honeywell EDA61K
//!         carried by operators on the move.
//!       - **SmartGlasses** — RealWear HMT-1, Vuzix Blade for
//!         hands-free assist with AR overlay.
//!       - **Pendant** — handheld machine pendant (FANUC iPendant,
//!         KUKA smartPAD); typically wired to one machine and
//!         smaller surface than a tablet.
//!
//!   * [`event::OperatorEvent`] — typed inbound interaction
//!     stream: tap, swipe, voice utterance, gaze-dwell (smart
//!     glasses), generic Acknowledge / Cancel. Operators emit
//!     these; the system polls them via `Hmi::pending_events`.
//!
//!   * [`hmi::Hmi`] — `Actuator` supertrait. Adds `hmi_id()`,
//!     `hmi_kind()`, `pending_events()`. Outbound announcements
//!     flow through the V1 `ActuatorCommand::Announce` variant
//!     added this session — same permit-gated dispatch path as
//!     every other Actuator command.
//!
//!   * [`mock::MockHmi`] — in-memory event queue + announcement
//!     log. Production replaces it with platform-specific impls
//!     (Tauri webview event bridge for touchscreens, RealWear
//!     SDK for smart glasses).
//!
//! ## Why pending_events instead of a stream
//! The trait returns `Vec<OperatorEvent>` from a non-streaming
//! poll method so it stays object-safe AND so the orchestrator
//! controls cadence. A push-based stream (tokio::broadcast) would
//! be richer but would force every HMI impl to manage subscriber
//! lifecycles; pull-based polling keeps the trait surface narrow
//! and matches the V11 `Scanner::last_scan` pattern.
//!
//! Continuous gesture streams (real-time AR gaze tracking) layer
//! on top of the trait via a separate event-bus crate in a later
//! session if a customer actually needs sub-100ms latency.
//!
//! ## Trust boundary
//! Announcements don't carry authorization. A handler that wants
//! to drive an operator action ("scan badge to unlock") emits
//! the Announce; the resulting badge scan goes through the
//! V11+V12 path and the interlock check happens there. The HMI
//! layer is the messaging fabric, not the policy engine.

pub mod event;
pub mod hmi;
pub mod kind;
pub mod mock;

pub use event::{OperatorEvent, SwipeDirection};
pub use hmi::Hmi;
pub use kind::HmiKind;
pub use mock::{EventQueueError, MockHmi};
