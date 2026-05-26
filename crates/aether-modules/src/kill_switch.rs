//! Module kill-switch state machine.
//!
//! A module can be remotely disabled in <2s by a Supabase Realtime
//! broadcast on `modules.status`. The network subscription is the TS
//! side; this is the pure-logic core it feeds: a per-module status
//! registry and the transition rules a broadcast triggers.
//!
//! ## States
//!  * `Enabled`  — normal; the runtime may load and run it.
//!  * `Disabled` — temporarily off (a bad release, a vendor outage). Can
//!    be re-enabled by a later signal.
//!  * `Revoked`  — permanently killed (compromised publisher key, malware
//!    finding). **Terminal**: it can never be re-enabled or merely
//!    disabled — that's the whole point of revocation. A fresh, re-signed
//!    module would be a new install, not a revival of this one.
//!
//! ## Default
//! A module the registry has never heard of is treated as runnable
//! (absence of a kill signal = not killed). The registry only records
//! deviations from "running".

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModuleStatus {
    Enabled,
    Disabled,
    Revoked,
}

/// Action carried by a kill-switch broadcast.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KillAction {
    Enable,
    Disable,
    Revoke,
}

/// Wire payload of a kill-switch broadcast (one row of `modules.status`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KillSignal {
    pub module_id: String,
    pub action: KillAction,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum KillError {
    /// Attempted to enable or disable a module that was already revoked.
    /// Revocation is terminal.
    #[error("module '{0}' is revoked; revocation is permanent")]
    Revoked(String),
}

/// Per-module kill-switch status registry. Only stores modules that have
/// deviated from the default running state.
#[derive(Clone, Debug, Default)]
pub struct ModuleRegistry {
    statuses: BTreeMap<String, ModuleStatus>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Explicit status, if the registry has a record. `None` = never seen
    /// (treated as runnable; see [`Self::is_runnable`]).
    pub fn status(&self, module_id: &str) -> Option<ModuleStatus> {
        self.statuses.get(module_id).copied()
    }

    /// Whether the runtime may load/run the module. Conservative only for
    /// explicit Disabled/Revoked; unknown modules default to runnable.
    pub fn is_runnable(&self, module_id: &str) -> bool {
        !matches!(
            self.statuses.get(module_id),
            Some(ModuleStatus::Disabled) | Some(ModuleStatus::Revoked)
        )
    }

    /// Apply a kill-switch broadcast, returning the resulting status.
    ///
    /// Transition rules:
    ///  * `Revoke` → `Revoked` from any state (terminal, idempotent).
    ///  * `Disable`/`Enable` on a revoked module → [`KillError::Revoked`].
    ///  * `Disable` → `Disabled`; `Enable` → `Enabled` otherwise.
    pub fn apply(&mut self, signal: &KillSignal) -> Result<ModuleStatus, KillError> {
        let current = self.statuses.get(&signal.module_id).copied();
        let next = match signal.action {
            KillAction::Revoke => ModuleStatus::Revoked,
            KillAction::Disable | KillAction::Enable if current == Some(ModuleStatus::Revoked) => {
                return Err(KillError::Revoked(signal.module_id.clone()));
            }
            KillAction::Disable => ModuleStatus::Disabled,
            KillAction::Enable => ModuleStatus::Enabled,
        };
        self.statuses.insert(signal.module_id.clone(), next);
        Ok(next)
    }

    /// Convenience wrappers around [`Self::apply`].
    pub fn disable(&mut self, module_id: &str) -> Result<ModuleStatus, KillError> {
        self.apply(&KillSignal {
            module_id: module_id.to_string(),
            action: KillAction::Disable,
        })
    }
    pub fn enable(&mut self, module_id: &str) -> Result<ModuleStatus, KillError> {
        self.apply(&KillSignal {
            module_id: module_id.to_string(),
            action: KillAction::Enable,
        })
    }
    pub fn revoke(&mut self, module_id: &str) -> Result<ModuleStatus, KillError> {
        self.apply(&KillSignal {
            module_id: module_id.to_string(),
            action: KillAction::Revoke,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_module_defaults_to_runnable() {
        let r = ModuleRegistry::new();
        assert!(r.is_runnable("com.aether.qc"));
        assert_eq!(r.status("com.aether.qc"), None);
    }

    #[test]
    fn disable_then_enable_round_trips() {
        let mut r = ModuleRegistry::new();
        assert_eq!(r.disable("m").unwrap(), ModuleStatus::Disabled);
        assert!(!r.is_runnable("m"));
        assert_eq!(r.enable("m").unwrap(), ModuleStatus::Enabled);
        assert!(r.is_runnable("m"));
    }

    #[test]
    fn revoke_makes_module_unrunnable() {
        let mut r = ModuleRegistry::new();
        assert_eq!(r.revoke("m").unwrap(), ModuleStatus::Revoked);
        assert!(!r.is_runnable("m"));
    }

    #[test]
    fn revocation_is_terminal() {
        let mut r = ModuleRegistry::new();
        r.revoke("m").unwrap();
        assert_eq!(r.enable("m"), Err(KillError::Revoked("m".into())));
        assert_eq!(r.disable("m"), Err(KillError::Revoked("m".into())));
        // Still revoked and unrunnable after the refused attempts.
        assert_eq!(r.status("m"), Some(ModuleStatus::Revoked));
        assert!(!r.is_runnable("m"));
    }

    #[test]
    fn revoke_is_idempotent_from_any_state() {
        let mut r = ModuleRegistry::new();
        r.disable("m").unwrap();
        assert_eq!(r.revoke("m").unwrap(), ModuleStatus::Revoked);
        assert_eq!(r.revoke("m").unwrap(), ModuleStatus::Revoked);
    }

    #[test]
    fn disabled_module_can_be_revoked() {
        let mut r = ModuleRegistry::new();
        r.disable("m").unwrap();
        assert_eq!(r.revoke("m").unwrap(), ModuleStatus::Revoked);
    }

    #[test]
    fn kill_signal_serde_round_trips() {
        let s = KillSignal {
            module_id: "com.aether.qc".into(),
            action: KillAction::Revoke,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"revoke\""));
        let back: KillSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn status_serde_is_kebab() {
        assert_eq!(
            serde_json::to_string(&ModuleStatus::Disabled).unwrap(),
            "\"disabled\""
        );
    }
}
