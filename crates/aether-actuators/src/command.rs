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
    /// V13: write a value to a named tag on a PLC / PAC / CNC /
    /// DCS controller. Distinct from MoveJoint because tags
    /// aren't a fixed-shape numeric address space — they're
    /// vendor-defined named symbols, and the value can be bool /
    /// int / float / text / bytes depending on the underlying
    /// data type. Reads don't have a CommandKind because they're
    /// side-effect-free and bypass the permit gate.
    WriteTag,
    /// V14: push an announcement / prompt / acknowledgement
    /// request to an HMI surface (touchscreen, rugged tablet,
    /// smart glasses, pendant). The HMI impl renders the
    /// content; operator interactions flow back through the
    /// non-dispatch `Hmi::pending_events` method.
    Announce,
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
            CommandKind::WriteTag => "write-tag",
            CommandKind::Announce => "announce",
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
    /// V13: write a typed value to a controller tag. `address` is
    /// the vendor-defined symbol (`"ns=2;s=Channel1.Device1.Coil0"`
    /// for OPC-UA, `"%MX0.0"` for IEC 61131, etc.) — the controller
    /// impl knows how to parse it. `value` carries the typed
    /// payload so a bool tag never gets a float and vice versa.
    /// Reads are NOT in the command enum because they bypass the
    /// permit gate; see `aether_controllers::Controller::read_tag`.
    WriteTag { address: String, value: TagValue },
    /// V14: render an announcement to an HMI surface. `severity`
    /// drives the visual treatment (Info banner, Warning toast,
    /// Critical full-screen, Emergency red-alert with haptic).
    /// `summary` is the one-line title; `body` is the optional
    /// detail string. HMI impls render based on their surface
    /// (touchscreen vs. smart-glasses AR vs. pendant 4-line LCD).
    Announce {
        severity: AnnounceSeverity,
        summary: String,
        body: Option<String>,
    },
}

/// Severity tiers for HMI announcements. Maps to ANSI/ISA-18.2
/// alarm priorities and the V5 `aether-safety::sos` tiering so
/// the same alarm taxonomy flows end-to-end from operator
/// notification through SOS escalation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnnounceSeverity {
    /// Informational — workflow progress, shift handover.
    Info,
    /// Warning — out-of-tolerance reading, missed maintenance
    /// window. Recoverable without intervention.
    Warning,
    /// Critical — equipment fault, safety interlock open,
    /// requires operator action.
    Critical,
    /// Emergency — imminent harm to people or product. Surfaces
    /// as a full-screen modal that can't be dismissed without
    /// supervisor unlock; cross-references the V5 SOS tier 1
    /// pipeline.
    Emergency,
}

impl AnnounceSeverity {
    pub fn slug(&self) -> &'static str {
        match self {
            AnnounceSeverity::Info => "info",
            AnnounceSeverity::Warning => "warning",
            AnnounceSeverity::Critical => "critical",
            AnnounceSeverity::Emergency => "emergency",
        }
    }
}

/// Typed value for a controller tag write. Mirrors the IEC 61131-3
/// elementary types that show up across OPC-UA / Modbus / EtherNet/
/// IP / S7. `Text` and `Bytes` cover the few extensions that show
/// up in PAC/CNC controllers (recipe names, raw register dumps).
/// Anything richer (structures, arrays) round-trips as `Bytes`
/// today; a future session may add struct support if a pilot
/// tenant actually needs it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum TagValue {
    Bool(bool),
    /// 64-bit signed integer — superset of IEC INT/DINT/LINT
    /// without lossy down-casts at the trait boundary.
    Int(i64),
    /// 64-bit IEEE float — superset of REAL/LREAL.
    Float(f64),
    /// UTF-8 text. Some vendors use this for recipe names and
    /// program selectors. Validate length against the tag's
    /// declared capacity at the impl level.
    Text(String),
    /// Opaque bytes for struct / array writes that don't decompose
    /// cleanly into the elementary types. The controller impl
    /// validates byte length against the tag's declared width.
    Bytes(Vec<u8>),
}

impl TagValue {
    /// Kebab-case discriminator slug. Persisted into the
    /// `actuator_commands.result` JSON so an auditor can filter
    /// "every bool write to a safety coil in the last 24 hours"
    /// without parsing the value field.
    pub fn slug(&self) -> &'static str {
        match self {
            TagValue::Bool(_) => "bool",
            TagValue::Int(_) => "int",
            TagValue::Float(_) => "float",
            TagValue::Text(_) => "text",
            TagValue::Bytes(_) => "bytes",
        }
    }
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
            ActuatorCommand::WriteTag { .. } => CommandKind::WriteTag,
            ActuatorCommand::Announce { .. } => CommandKind::Announce,
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
            CommandKind::WriteTag,
            CommandKind::Announce,
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
    fn write_tag_variant_round_trips_through_serde() {
        // V13: pin wire format for the controller-side variant.
        let cmd = ActuatorCommand::WriteTag {
            address: "ns=2;s=Channel1.Device1.Coil0".into(),
            value: TagValue::Bool(true),
        };
        let s = serde_json::to_string(&cmd).unwrap();
        assert!(s.contains("\"kind\":\"write-tag\""));
        assert!(s.contains("\"type\":\"bool\""));
        assert!(s.contains("\"value\":true"));
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
        assert_eq!(cmd.kind(), CommandKind::WriteTag);
        assert_eq!(cmd.kind().slug(), "write-tag");
    }

    #[test]
    fn tag_value_slug_is_kebab_case_for_every_variant() {
        // Slug lands in the audit row JSON; renames are schema
        // breakage.
        assert_eq!(TagValue::Bool(true).slug(), "bool");
        assert_eq!(TagValue::Int(42).slug(), "int");
        assert_eq!(TagValue::Float(1.5).slug(), "float");
        assert_eq!(TagValue::Text("x".into()).slug(), "text");
        assert_eq!(TagValue::Bytes(vec![1, 2]).slug(), "bytes");
    }

    #[test]
    fn tag_value_serde_uses_externally_tagged_type_and_value() {
        // Pin the JSON layout — `{"type": "<slug>", "value": <v>}`
        // — so Edge Functions can pattern-match without knowing
        // the variant set in advance.
        let v = serde_json::to_value(TagValue::Float(2.5)).unwrap();
        assert_eq!(v, serde_json::json!({"type": "float", "value": 2.5}));
        let back: TagValue = serde_json::from_value(v).unwrap();
        assert_eq!(back, TagValue::Float(2.5));

        let v = serde_json::to_value(TagValue::Text("recipe-A".into())).unwrap();
        assert_eq!(v, serde_json::json!({"type": "text", "value": "recipe-A"}));
    }

    #[test]
    fn announce_variant_round_trips_through_serde() {
        // V14: pin the HMI command wire format.
        let cmd = ActuatorCommand::Announce {
            severity: AnnounceSeverity::Critical,
            summary: "spindle fault on M-204".into(),
            body: Some("axis Z servo timeout, fault code 4732".into()),
        };
        let s = serde_json::to_string(&cmd).unwrap();
        assert!(s.contains("\"kind\":\"announce\""));
        assert!(s.contains("\"severity\":\"critical\""));
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
        assert_eq!(cmd.kind(), CommandKind::Announce);
        assert_eq!(cmd.kind().slug(), "announce");
    }

    #[test]
    fn announce_body_is_optional_for_short_form_announcements() {
        let cmd = ActuatorCommand::Announce {
            severity: AnnounceSeverity::Info,
            summary: "shift handover at 14:00".into(),
            body: None,
        };
        let s = serde_json::to_string(&cmd).unwrap();
        let back: ActuatorCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn announce_severity_slug_matches_iso_alarm_taxonomy() {
        // Slugs map to the V5 SOS tier names + ANSI/ISA-18.2
        // alarm priorities. Pin them so a rename doesn't desync
        // from the SOS escalation pipeline.
        assert_eq!(AnnounceSeverity::Info.slug(), "info");
        assert_eq!(AnnounceSeverity::Warning.slug(), "warning");
        assert_eq!(AnnounceSeverity::Critical.slug(), "critical");
        assert_eq!(AnnounceSeverity::Emergency.slug(), "emergency");
    }

    #[test]
    fn tag_value_bool_and_int_with_same_underlying_zero_serialize_distinctly() {
        // `TagValue::Bool(false)` and `TagValue::Int(0)` must
        // produce different JSON — the type tag carries the
        // distinction so the controller impl can route to the
        // right backing typed write.
        let b = serde_json::to_string(&TagValue::Bool(false)).unwrap();
        let i = serde_json::to_string(&TagValue::Int(0)).unwrap();
        assert_ne!(b, i);
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
