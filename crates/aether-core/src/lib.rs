//! Shared domain primitives used across every AETHER-OS crate.
//!
//! Keep this crate small and dependency-light: anything that lands here
//! ends up in every binary. Reach for crate-specific types in their
//! respective crates (e.g. `aether-sync::Hlc` vs the re-export here).

pub mod error;
pub mod ids;
pub mod time;

pub use error::{Error, Result};
pub use ids::{
    ActuatorId, CameraId, ControllerId, EntityId, HmiId, MachineId, MaterialId, ModuleId, RobotId,
    ScannerId, TenantId, UserId, WorkOrderId,
};
pub use time::Hlc;
