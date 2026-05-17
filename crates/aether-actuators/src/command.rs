//! Structured commands that an [`crate::Actuator`] can dispatch.
//!
//! ## Why a sum-type, not bytes
//! The original `aether-protocols::Bridge::write(tag: &str, value: f64)`
//! is shaped for scalar PLC writes. Vision and robotics commands carry
//! richer structure — a `MoveJoint` is `(joint_index, target_radians)`,
//! a `Capture` carries quality + format, a `DispatchJob` carries route
//! and priority. Forcing all of these into `(tag, f64)` would lose
//! structure and force every Actuator impl to re-parse strings.
//!
//! The trade-off: every new command shape adds a variant here, which
//! every concrete Actuator impl must match exhaustively. That's the
//! desired property — adding a variant is a deliberate workspace-wide
//! change, not a silent extension.
//!
//! ## Why a flat `CommandKind` tag
//! Audit rows and ledger entries log the *kind* of command independently
//! of the payload. Carrying [`CommandKind`] alongside the variant keeps
//! the kebab-case string stable across schema migrations even if the
//! payload structure evolves.

use serde::{Deserialize, Serialize};

/// Kebab-case tag for [`ActuatorCommand`] variants. Persisted into the
/// `actuator_commands` audit table and into log lines, so the wire
/// format here is load-bearing — renaming a variant is a schema
/// migration, not a refactor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandKind {
    Capture,
    MoveJoint,
    MoveLinear,
    DispatchJob,
    Halt,
    /// V11: scan a barcode / QR / RFID / NFC token. The auto-id
    /// hardware reports the decoded payload + class via
    /// `ActuatorResult.data`. Distinct from `Capture` because the
    /// return shape is a structured payload, not an image — and
    /// because a camera-based QR scan and an RFID antenna read
    /// have to share one command surface.
    Scan,
}

impl CommandKind {
    pub fn slug(&self) -> &'static str {
        match self {
            CommandKind::Capture => "capture",
            CommandKind::MoveJoint => "move-joint",
            CommandKind::MoveLinear => "move-linear",
            CommandKind::DispatchJob => "dispatch-job",
            CommandKind::Halt => "halt",
            CommandKind::Scan => "scan",
        }
    }
}

/// Concrete commands an Actuator can execute. Implementations match on
/// the variant they support and return
/// [`crate::actuator::ActuatorError::BadCommand`] for variants they
/// don't (a Camera should never receive `MoveJoint`, etc.).
///
/// `Halt` is the universal pre-emption command — every Actuator MUST
/// honor it regardless of category. It bypasses normal TTL/permit
/// semantics in implementations that wire to a hardware e-stop line.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActuatorCommand {
    /// Capture one frame from a camera. `quality` 0..=100 maps to the
    /// underlying codec's quality knob. `format` is a hint; the camera
    /// may downgrade (e.g. PNG → JPEG) if the requested format isn't
    /// supported.
    Capture { quality: u8, format: String },
    /// Move a single joint to a target absolute angle (radians). For
    /// arms, joint indices are 0..N-1 (typical N = 6). For AGVs, joint
    /// indices map to (0=x, 1=y, 2=heading).
    MoveJoint { joint: u8, target_rad: f64 },
    /// Move the tool-center-point along a straight line in Cartesian
    /// space. `xyz` in meters, `rxryrz` in radians. The Actuator
    /// computes the inverse kinematics.
    MoveLinear {
        xyz: [f64; 3],
        rxryrz: [f64; 3],
        speed_mps: f64,
    },
    /// Dispatch an AGV/AMR job: drive to `destination` via the named
    /// route plan, then await further instructions. `priority` 0..=9.
    DispatchJob {
        route_id: String,
        destination: String,
        priority: u8,
    },
    /// Universal pre-emption — stop now. Every Actuator MUST honor.
    Halt,
    /// V11: trigger a scan from auto-id hardware (barcode reader,
    /// QR camera, RFID antenna, NFC pad). `trigger` distinguishes
    /// operator-pulled (Manual), software-driven cycle (Auto), and
    /// background continuous (Continuous) reads. Scanners not in
    /// continuous mode reject `Continuous`; fixed-mode industrial
    /// readers reject `Manual`.
    Scan { trigger: ScanTrigger },
}

/// How the scan was initiated. Shapes the scanner's behavior:
/// `Manual` triggers a single one-shot read on an interactive
/// device (handheld scanner trigger pull); `Auto` is software-
/// driven (a workflow step requests a confirmation scan); a
/// `Continuous` mode reader (fixed industrial antenna) starts /
/// remains in always-on mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanTrigger {
    Manual,
    Auto,
    Continuous,
}

impl ActuatorCommand {
    pub fn kind(&self) -> CommandKind {
        match self {
            ActuatorCommand::Capture { .. } => CommandKind::Capture,
            ActuatorCommand::MoveJoint { .. } => CommandKind::MoveJoint,
            ActuatorCommand::MoveLinear { .. } => CommandKind::MoveLinear,
            ActuatorCommand::DispatchJob { .. } => CommandKind::DispatchJob,
            ActuatorCommand::Halt => CommandKind::Halt,
            ActuatorCommand::Scan { .. } => CommandKind::Scan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_is_stable_across_payload_shape() {
        // The whole point of carrying CommandKind separately from the
        // enum variant: an audit row that says `kind = "move-joint"`
        // stays valid even when MoveJoint adds new fields later.
        let cmd = ActuatorCommand::MoveJoint {
            joint: 2,
            target_rad: 1.57,
        };
        assert_eq!(cmd.kind(), CommandKind::MoveJoint);
        assert_eq!(cmd.kind().slug(), "move-joint");
    }

    #[test]
    fn slugs_are_distinct_and_kebab_cased() {
        // The slug is persisted; collisions or non-kebab spelling
        // would corrupt the audit schema.
        let kinds = [
            CommandKind::Capture,
            CommandKind::MoveJoint,
            CommandKind::MoveLinear,
            CommandKind::DispatchJob,
            CommandKind::Halt,
            CommandKind::Scan,
        ];
        let mut slugs: Vec<&'static str> = kinds.iter().map(|k| k.slug()).collect();
        slugs.sort_unstable();
        let before = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), before, "all slugs distinct");
        for s in &slugs {
            assert!(!s.contains('_'), "kebab-case: {s}");
            assert_eq!(s.to_lowercase(), **s, "lowercase: {s}");
        }
    }

    #[test]
    fn scan_variant_round_trips_through_serde() {
        // V11: pin the wire format for the new Scan variant.
        let cmd = ActuatorCommand::Scan {
            trigger: ScanTrigger::Manual,
        };
        let s = serde_json::to_string(&cmd).unwrap();
        assert!(s.contains("\"kind\":\"scan\""));
        assert!(s.contains("\"trigger\":\"manual\""));
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
        assert_eq!(cmd.kind(), CommandKind::Scan);
        assert_eq!(cmd.kind().slug(), "scan");
    }

    #[test]
    fn scan_trigger_serde_matches_kebab_case() {
        // Wire format pinned — Edge Functions and frontend
        // pattern-match on the kebab values, so renames here are
        // cross-stack breakage.
        let manual = serde_json::to_string(&ScanTrigger::Manual).unwrap();
        let auto = serde_json::to_string(&ScanTrigger::Auto).unwrap();
        let cont = serde_json::to_string(&ScanTrigger::Continuous).unwrap();
        assert_eq!(manual, "\"manual\"");
        assert_eq!(auto, "\"auto\"");
        assert_eq!(cont, "\"continuous\"");
    }

    #[test]
    fn capture_serde_round_trip() {
        // Wire-format stability — the `tag` field on the enum keeps
        // the JSON layout discoverable for non-Rust consumers.
        let cmd = ActuatorCommand::Capture {
            quality: 85,
            format: "png".into(),
        };
        let s = serde_json::to_string(&cmd).unwrap();
        assert!(s.contains("\"kind\":\"capture\""));
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn halt_has_no_payload_but_serializes_with_kind_tag() {
        let cmd = ActuatorCommand::Halt;
        let s = serde_json::to_string(&cmd).unwrap();
        assert!(s.contains("halt"));
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ActuatorCommand::Halt);
    }
}
