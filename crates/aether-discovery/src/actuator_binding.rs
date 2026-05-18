//! Suggest the right `Actuator` kind for a discovered device.
//!
//! ## What this closes
//! V9 + V16 probes surface a `DiscoveredDevice` tagged with a
//! `ProbeKind`. The actuator crates (V1–V19) expose typed
//! `Actuator`-implementing types. Between them sits the
//! binding wizard's first decision: "given a Vision-probe
//! response, do I construct a `MockCamera`, an
//! `OpcUaCamera`, …?"
//!
//! This module is the deterministic mapping that drives the
//! wizard's default selection. The operator can still
//! override; the suggestion is a starting point, not a policy.
//!
//! ## What this doesn't do
//! Construct the actuator. Construction needs a transport
//! config (RTSP URL, gRPC endpoint, …) that the wizard
//! collects after the kind is settled. This module's output
//! is purely advisory.

use crate::probe::ProbeKind;
use crate::result::DiscoveredDevice;
use crate::suggestion::Confidence;
use serde::{Deserialize, Serialize};

/// Canonical actuator-kind slug. Matches
/// `aether_actuators::Actuator::actuator_kind()` so a
/// downstream factory can switch on this string to pick the
/// right concrete impl. Renaming a variant here is a
/// cross-crate breakage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuggestedActuatorKind {
    Camera,
    RobotArm,
    Agv,
    Scanner,
    Controller,
    Hmi,
    Gateway,
    /// Falls back to the generic `MockActuator` for kinds we
    /// don't yet have a specialized impl for.
    Generic,
}

impl SuggestedActuatorKind {
    pub fn slug(&self) -> &'static str {
        match self {
            SuggestedActuatorKind::Camera => "camera",
            SuggestedActuatorKind::RobotArm => "robot-arm",
            SuggestedActuatorKind::Agv => "agv",
            SuggestedActuatorKind::Scanner => "scanner",
            SuggestedActuatorKind::Controller => "controller",
            SuggestedActuatorKind::Hmi => "hmi",
            SuggestedActuatorKind::Gateway => "gateway",
            SuggestedActuatorKind::Generic => "generic",
        }
    }
}

/// The suggestion the wizard renders next to the discovered
/// device row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActuatorBindingSuggestion {
    pub actuator_kind: SuggestedActuatorKind,
    /// Certs the operator will be prompted to assign as
    /// `machine_required_certs` for the bound actuator. Pulled
    /// from the per-kind defaults; the operator can edit.
    pub required_cert_codes: Vec<&'static str>,
    pub confidence: Confidence,
    /// Human-readable rationale rendered next to the
    /// suggestion in the binding wizard. Pinned-string-keyed
    /// for downstream i18n.
    pub rationale: &'static str,
}

/// Map a discovered device to the recommended actuator kind +
/// cert defaults. Deterministic — same `ProbeKind` always
/// yields the same suggestion.
///
/// The mapping is intentionally coarse (one probe → one kind)
/// at this layer. Vendor-specific refinement (Siemens vs
/// Allen-Bradley for the same OPC-UA probe) lives in a
/// future vendor-template layer that consumes the
/// `DiscoveredDevice::metadata.vendor` field this function
/// passes through unchanged.
pub fn suggest_actuator(device: &DiscoveredDevice) -> ActuatorBindingSuggestion {
    match device.probe {
        ProbeKind::Vision => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Camera,
            required_cert_codes: vec!["operator", "vision-bind"],
            confidence: Confidence::Medium,
            rationale: "RTSP/ONVIF response — bind as camera; operator + vision-bind certs typical",
        },
        ProbeKind::Robot => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::RobotArm,
            required_cert_codes: vec!["robot-operator", "estop-trained"],
            confidence: Confidence::Medium,
            rationale: "UR Dashboard / robot-control port — bind as robot arm; estop-trained cert is load-bearing",
        },
        ProbeKind::Scanner => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Scanner,
            required_cert_codes: vec!["shop-floor"],
            confidence: Confidence::Medium,
            rationale: "LLRP / barcode-reader response — bind as scanner; shop-floor cert suffices",
        },
        ProbeKind::Hmi => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Hmi,
            required_cert_codes: vec!["operator"],
            confidence: Confidence::Medium,
            rationale: "VNC / HMI port — bind as HMI surface; operator cert default",
        },
        ProbeKind::Gateway => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Gateway,
            required_cert_codes: vec!["gateway-admin"],
            confidence: Confidence::Medium,
            rationale: "Industrial-gateway port — bind as gateway; gateway-admin cert required",
        },
        ProbeKind::OpcUa => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Controller,
            required_cert_codes: vec!["plc-operator"],
            confidence: Confidence::High,
            rationale: "OPC-UA endpoint — bind as PLC controller; plc-operator cert required for tag writes",
        },
        ProbeKind::Modbus | ProbeKind::EthernetIp => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Controller,
            required_cert_codes: vec!["plc-operator"],
            confidence: Confidence::Medium,
            rationale: "Industrial PLC protocol — bind as controller; plc-operator cert required",
        },
        ProbeKind::Mqtt => ActuatorBindingSuggestion {
            actuator_kind: SuggestedActuatorKind::Gateway,
            required_cert_codes: vec!["gateway-admin"],
            confidence: Confidence::Low,
            rationale: "MQTT broker — could be a gateway or telemetry hub; bind as gateway and refine",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::VendorMetadata;
    use chrono::Utc;

    fn device(kind: ProbeKind) -> DiscoveredDevice {
        DiscoveredDevice {
            fingerprint: format!("127.0.0.1:9000/{}", kind.slug()),
            host: "127.0.0.1".into(),
            port: 9000,
            probe: kind,
            metadata: VendorMetadata {
                vendor: None,
                model: None,
                firmware: None,
                extras: serde_json::json!({}),
            },
            suggested_bindings: vec![],
            discovered_at: Utc::now(),
        }
    }

    #[test]
    fn slugs_distinct_and_kebab_cased() {
        let kinds = [
            SuggestedActuatorKind::Camera,
            SuggestedActuatorKind::RobotArm,
            SuggestedActuatorKind::Agv,
            SuggestedActuatorKind::Scanner,
            SuggestedActuatorKind::Controller,
            SuggestedActuatorKind::Hmi,
            SuggestedActuatorKind::Gateway,
            SuggestedActuatorKind::Generic,
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
    fn vision_probe_suggests_camera() {
        let s = suggest_actuator(&device(ProbeKind::Vision));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::Camera);
        assert_eq!(s.actuator_kind.slug(), "camera");
        assert!(s.required_cert_codes.contains(&"operator"));
    }

    #[test]
    fn robot_probe_suggests_robot_arm_with_estop_cert() {
        let s = suggest_actuator(&device(ProbeKind::Robot));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::RobotArm);
        // The estop-trained cert is the safety-critical one;
        // pin it explicitly so a future refactor that drops
        // it surfaces here.
        assert!(s.required_cert_codes.contains(&"estop-trained"));
    }

    #[test]
    fn scanner_probe_suggests_scanner() {
        let s = suggest_actuator(&device(ProbeKind::Scanner));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::Scanner);
    }

    #[test]
    fn hmi_probe_suggests_hmi() {
        let s = suggest_actuator(&device(ProbeKind::Hmi));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::Hmi);
    }

    #[test]
    fn gateway_probe_suggests_gateway() {
        let s = suggest_actuator(&device(ProbeKind::Gateway));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::Gateway);
    }

    #[test]
    fn opcua_suggests_controller_with_high_confidence() {
        let s = suggest_actuator(&device(ProbeKind::OpcUa));
        assert_eq!(s.actuator_kind, SuggestedActuatorKind::Controller);
        // OPC-UA is unambiguous (vs Modbus which could be a
        // standalone device); pin High.
        assert_eq!(s.confidence, Confidence::High);
    }

    #[test]
    fn modbus_and_ethernet_ip_also_suggest_controller() {
        for kind in [ProbeKind::Modbus, ProbeKind::EthernetIp] {
            let s = suggest_actuator(&device(kind));
            assert_eq!(s.actuator_kind, SuggestedActuatorKind::Controller);
        }
    }

    #[test]
    fn mqtt_suggests_gateway_with_low_confidence() {
        // MQTT is genuinely ambiguous — could be a gateway,
        // could be a telemetry hub, could be a standalone
        // sensor publishing direct. Low confidence flags the
        // operator to inspect.
        let s = suggest_actuator(&device(ProbeKind::Mqtt));
        assert_eq!(s.confidence, Confidence::Low);
    }

    #[test]
    fn suggestion_is_deterministic_per_probe_kind() {
        // Pin determinism — same device kind always yields
        // the same suggestion. Lets the wizard cache the
        // default UI tile per discovered fingerprint.
        let d = device(ProbeKind::Vision);
        let a = suggest_actuator(&d);
        let b = suggest_actuator(&d);
        assert_eq!(a.actuator_kind, b.actuator_kind);
        assert_eq!(a.required_cert_codes, b.required_cert_codes);
        assert_eq!(a.confidence, b.confidence);
    }

    #[test]
    fn every_probe_kind_has_at_least_one_required_cert() {
        // Safety guardrail: every discovered device suggestion
        // must include at least one cert. A binding with NO
        // required certs would mean any operator could
        // dispatch commands — that's a regression we want to
        // catch loudly.
        for kind in [
            ProbeKind::OpcUa,
            ProbeKind::Mqtt,
            ProbeKind::Modbus,
            ProbeKind::EthernetIp,
            ProbeKind::Vision,
            ProbeKind::Robot,
            ProbeKind::Scanner,
            ProbeKind::Hmi,
            ProbeKind::Gateway,
        ] {
            let s = suggest_actuator(&device(kind));
            assert!(
                !s.required_cert_codes.is_empty(),
                "{kind:?} has no required certs — would allow unguarded dispatch"
            );
        }
    }

    #[test]
    fn suggested_kind_serde_uses_kebab_case() {
        let s = serde_json::to_string(&SuggestedActuatorKind::RobotArm).unwrap();
        assert_eq!(s, "\"robot-arm\"");
        let back: SuggestedActuatorKind = serde_json::from_str("\"robot-arm\"").unwrap();
        assert_eq!(back, SuggestedActuatorKind::RobotArm);
    }
}
