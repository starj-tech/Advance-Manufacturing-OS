//! Module Injection runtime support.
//!
//! Modules are TOML manifests + signed bundles. The host validates the
//! signature against per-tenant trusted publisher keys, then loads the
//! UI bundle in a Web Worker (TS side) and optionally runs a WASI
//! backend module via wasmtime (PR #4).

pub mod capability;
pub mod kill_switch;
pub mod loader;
pub mod manifest;
pub mod verify;

pub use capability::{
    CapabilityError, CapabilityGate, CapabilityKind, CapabilityPolicy, CapabilityRequest,
};
pub use kill_switch::{KillAction, KillError, KillSignal, ModuleRegistry, ModuleStatus};
pub use loader::{LoadError, LoadedModule};
pub use manifest::{
    Manifest, ManifestEntry, ManifestError, ManifestModule, ManifestPermissions, ManifestSig,
};
pub use verify::{verify_bundle, VerifyError};
