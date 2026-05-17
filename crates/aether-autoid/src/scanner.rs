//! `Scanner` — auto-id hardware as an
//! [`aether_actuators::Actuator`].
//!
//! ## Why supertrait over `Actuator`
//! Scanners share the V1 permit gating + e-stop preemption with
//! cameras and robots. Reusing the trait surface means:
//!   * Every scan call goes through `gate()` → permit → dispatch
//!     in the same shape as a camera capture or a robot move.
//!   * E-stop preemption via `dispatch_with_estop` works for
//!     scanners without per-impl plumbing.
//!   * The `actuator_commands` audit ledger (V10) gets scan rows
//!     for free with `command_kind = 'scan'`.
//!
//! ## What `Scanner` adds beyond `Actuator`
//!   * `scanner_id()` — typed ID for cross-table joins. The
//!     `ActuatorId` superset still uniquely identifies the
//!     device; `ScannerId` is a domain-specific projection.
//!   * `last_scan()` — atomic snapshot of the most recent
//!     payload. Pipeline code that wants the answer without
//!     dispatching a fresh Scan reads this instead.
//!
//! Returning `Option<ScannedPayload>` from `last_scan` rather than
//! a `Vec<ScannedPayload>` keeps the trait object-safe and matches
//! the production semantic: scanners overwrite, not accumulate.
//! Multi-scan history lives in the audit ledger, not the trait.

use crate::scan::ScannedPayload;
use aether_actuators::Actuator;
use aether_core::ScannerId;
use async_trait::async_trait;

#[async_trait]
pub trait Scanner: Actuator {
    /// Stable identity for this scanner. Distinct from the
    /// underlying `ActuatorId` superset so cross-table joins on
    /// `scans.scanner_id` use the right column.
    fn scanner_id(&self) -> ScannerId;

    /// Most recent payload, if the scanner has read anything.
    /// `None` on a freshly-bound scanner or after `Halt`. Cheap
    /// (typically a `Mutex` snapshot) so callers can poll without
    /// concern.
    async fn last_scan(&self) -> Option<ScannedPayload>;
}
