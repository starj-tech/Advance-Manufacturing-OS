//! `ControllerKind` — the four controller families we route
//! commands to.
//!
//! ## Why an enum rather than just a string
//! Different controller families have different default transports
//! and different command semantics. A CNC's `LoadProgram` is
//! meaningless to a DCS; a DCS's batch-recipe write is meaningless
//! to a PLC. A typed kind lets the trait surface stay narrow
//! (Controller trait works for all four) while binding-time UI
//! and routing code branches cleanly. Adding a kind is a
//! deliberate workspace-wide change — the slug is persisted into
//! the audit ledger and module-side TOML.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerKind {
    /// Programmable Logic Controller — discrete I/O, ladder
    /// logic, fast scan (10–100 Hz typical). Siemens S7,
    /// Allen-Bradley ControlLogix, Beckhoff TwinCAT.
    Plc,
    /// Programmable Automation Controller — PLC with built-in
    /// motion / vision / networking. Rockwell CompactLogix,
    /// Schneider M580.
    Pac,
    /// CNC for machine tools — runs G-code, manages spindles +
    /// tool changers + axes. FANUC, Siemens Sinumerik,
    /// Heidenhain.
    Cnc,
    /// Distributed Control System for process plants — batch +
    /// continuous control. Emerson DeltaV, Honeywell Experion.
    Dcs,
}

impl ControllerKind {
    /// Kebab-case slug for the audit ledger + module TOML.
    /// Renaming a variant is a schema migration.
    pub fn slug(&self) -> &'static str {
        match self {
            ControllerKind::Plc => "plc",
            ControllerKind::Pac => "pac",
            ControllerKind::Cnc => "cnc",
            ControllerKind::Dcs => "dcs",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let kinds = [
            ControllerKind::Plc,
            ControllerKind::Pac,
            ControllerKind::Cnc,
            ControllerKind::Dcs,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|k| k.slug()).collect();
        let before = slugs.len();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), before);
        for s in &slugs {
            assert!(!s.contains('_'));
            assert_eq!(s.to_lowercase(), **s);
        }
    }

    #[test]
    fn kind_serde_matches_kebab_case() {
        // Wire format pinned.
        let cases = [
            (ControllerKind::Plc, "\"plc\""),
            (ControllerKind::Pac, "\"pac\""),
            (ControllerKind::Cnc, "\"cnc\""),
            (ControllerKind::Dcs, "\"dcs\""),
        ];
        for (k, expected) in cases {
            let s = serde_json::to_string(&k).unwrap();
            assert_eq!(s, expected);
            let back: ControllerKind = serde_json::from_str(expected).unwrap();
            assert_eq!(back, k);
        }
    }
}
