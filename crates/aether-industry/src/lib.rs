//! Universal Module — industry-specific logic injection.
//!
//! When a tenant declares its industry vertical, AETHER-OS automatically
//! activates a curated set of capabilities tailored to that industry —
//! no custom code, no per-customer fork. A food & beverage plant gets
//! `Expired Date Tracking` and `Cold Chain Monitor`; an automotive plant
//! gets `Parts Serial Tracking` and `Precision Calibration`.
//!
//! ## Design
//! `IndustryProfile` is a manifest-style descriptor that lists:
//!   - **modules** — IDs from the module registry to auto-install
//!   - **capabilities** — feature flags surfaced to the UI as
//!     `useCapability("expired-date-tracking")` checks
//!   - **standards** — compliance standards typically applicable, used
//!     as the seed for the `aether-compliance` policy set
//!
//! 21 verticals ship in this PR, covering most of global discrete +
//! process manufacturing. Tenants can override individual capabilities
//! per-site without forking the profile.
//!
//! ## Skeleton scope
//! Profiles are defined in code as a const table (no DB lookups) so the
//! industry assignment is deterministic and reproducible. Per-tenant
//! overrides land in PR #6.

pub mod capability;
pub mod industry;
pub mod profile;

pub use capability::{Capability, CapabilityKind};
pub use industry::{Industry, INDUSTRIES};
pub use profile::{profile_for, IndustryProfile, ProfileError};
