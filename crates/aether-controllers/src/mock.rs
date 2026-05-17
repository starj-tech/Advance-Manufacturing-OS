//! [`MockController`] — in-memory tag map with strict type
//! checking on read/write.
//!
//! Pipeline tests declare tags up-front (address + initial typed
//! value), then read / write against the map. The mock enforces
//! the same "declared first, then accessed" contract that a real
//! controller would (no silent zero values for unknown addresses).
//!
//! ## Type strictness
//! A tag declared `TagValue::Bool(false)` rejects writes of any
//! other variant — same as a real PLC where the address backs a
//! specific data type. This catches binding bugs in test rather
//! than letting them through to a deploy where the real PLC
//! returns "BadConversion" at runtime.

use crate::controller::{Controller, ReadError, WriteError};
use crate::kind::ControllerKind;
use crate::tag::TagAddress;
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::{ActuatorCommand, TagValue};
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, ControllerId};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum TagDeclarationError {
    /// Same address declared twice. Catches the test-fixture bug
    /// of looping `declare_tag` with a typo in the address that
    /// silently overrides the previous declaration.
    #[error("tag {0} already declared")]
    DuplicateAddress(String),
}

/// Declarative tag spec — address + initial value (which also
/// pins the type).
#[derive(Clone, Debug)]
pub struct TagDeclaration {
    pub address: TagAddress,
    pub initial: TagValue,
}

pub struct MockController {
    actuator_id: ActuatorId,
    controller_id: ControllerId,
    kind: ControllerKind,
    /// Map address → current value. The value's variant pins the
    /// type; writes that don't match get rejected.
    tags: Mutex<HashMap<TagAddress, TagValue>>,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockController {
    pub fn new(kind: ControllerKind) -> Self {
        Self {
            actuator_id: ActuatorId::new(),
            controller_id: ControllerId::new(),
            kind,
            tags: Mutex::new(HashMap::new()),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Declare a tag with its initial value. Tags MUST be
    /// declared before they can be read or written; surfaces
    /// `DuplicateAddress` on a repeat declaration so a test
    /// fixture bug doesn't silently lose state.
    pub async fn declare_tag(&self, decl: TagDeclaration) -> Result<(), TagDeclarationError> {
        let mut tags = self.tags.lock().await;
        if tags.contains_key(&decl.address) {
            return Err(TagDeclarationError::DuplicateAddress(
                decl.address.as_str().to_string(),
            ));
        }
        tags.insert(decl.address, decl.initial);
        Ok(())
    }

    /// Snapshot the commands received so far. Mirrors the audit-
    /// uniformity pattern from V1-V12: rejected commands also
    /// appear here so an investigator sees what was attempted.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }

    /// Number of declared tags. Useful for asserting "test
    /// declared N tags before the run."
    pub async fn tag_count(&self) -> usize {
        self.tags.lock().await.len()
    }
}

#[async_trait]
impl Actuator for MockController {
    fn actuator_id(&self) -> ActuatorId {
        self.actuator_id
    }
    fn actuator_kind(&self) -> &'static str {
        "controller"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::WriteTag { address, value } => {
                // Validate address shape; surface as BadCommand
                // rather than panicking on a malformed input.
                let typed = TagAddress::new(address.clone())
                    .map_err(|e| ActuatorError::BadCommand(format!("address: {e}")))?;
                let mut tags = self.tags.lock().await;
                let current = tags
                    .get(&typed)
                    .ok_or_else(|| ActuatorError::BadCommand(format!("unknown tag: {address}")))?;
                if current.slug() != value.slug() {
                    return Err(ActuatorError::BadCommand(format!(
                        "type mismatch on {address}: declared {} attempted {}",
                        current.slug(),
                        value.slug()
                    )));
                }
                let prev = tags.insert(typed.clone(), value.clone()).unwrap();
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "write-tag".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({
                        "address": address,
                        "type": value.slug(),
                        "previous_type": prev.slug(),
                    }),
                })
            }
            ActuatorCommand::Halt => {
                // Halt on a controller is acknowledged but
                // doesn't tear down the tag map — real PLC e-stop
                // typically pauses scan, not erases program. The
                // mock mirrors that observable.
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id,
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "controller does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl Controller for MockController {
    fn controller_id(&self) -> ControllerId {
        self.controller_id
    }
    fn controller_kind(&self) -> ControllerKind {
        self.kind
    }
    async fn read_tag(&self, address: &TagAddress) -> Result<TagValue, ReadError> {
        self.tags
            .lock()
            .await
            .get(address)
            .cloned()
            .ok_or_else(|| ReadError::UnknownTag(address.as_str().to_string()))
    }
}

/// Helper for tests / production callers that prefer the typed
/// WriteError surface. Wraps the dispatch path and rewrites the
/// V1 `ActuatorError::BadCommand` strings into the typed
/// `WriteError` variants the controller crate defines. Optional
/// — callers happy with `ActuatorError` can dispatch directly.
pub async fn typed_write(
    controller: &dyn Controller,
    address: TagAddress,
    value: TagValue,
    permit: ActuatorPermit,
) -> Result<(), WriteError> {
    let cmd = ActuatorCommand::WriteTag {
        address: address.as_str().to_string(),
        value: value.clone(),
    };
    match controller.dispatch(cmd, permit).await {
        Ok(_) => Ok(()),
        Err(ActuatorError::BadCommand(s)) => {
            // The mock surfaces type-mismatch + unknown-tag as
            // BadCommand strings. Translate the most common
            // shapes back into typed variants; anything else
            // stays as VendorRefused so callers see the
            // underlying message.
            if s.starts_with("unknown tag:") {
                Err(WriteError::UnknownTag(
                    s.trim_start_matches("unknown tag: ").to_string(),
                ))
            } else if s.starts_with("type mismatch") {
                Err(WriteError::TypeMismatch {
                    tag: address.as_str().to_string(),
                    declared: "see message",
                    attempted: value.slug(),
                })
            } else {
                Err(WriteError::VendorRefused(s))
            }
        }
        Err(e) => Err(WriteError::Transport(format!("{e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_actuators::command::CommandKind;
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use uuid::Uuid;

    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "plc-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["plc-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock approves in test fixture")
    }

    fn addr(s: &str) -> TagAddress {
        TagAddress::new(s).unwrap()
    }

    #[tokio::test]
    async fn fresh_controller_has_zero_tags() {
        let c = MockController::new(ControllerKind::Plc);
        assert_eq!(c.tag_count().await, 0);
    }

    #[tokio::test]
    async fn declare_and_read_round_trips() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MX0.0"),
            initial: TagValue::Bool(true),
        })
        .await
        .unwrap();
        let v = c.read_tag(&addr("%MX0.0")).await.unwrap();
        assert_eq!(v, TagValue::Bool(true));
    }

    #[tokio::test]
    async fn duplicate_declaration_is_rejected() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MX0.0"),
            initial: TagValue::Bool(true),
        })
        .await
        .unwrap();
        let err = c
            .declare_tag(TagDeclaration {
                address: addr("%MX0.0"),
                initial: TagValue::Bool(false),
            })
            .await
            .unwrap_err();
        assert!(matches!(err, TagDeclarationError::DuplicateAddress(_)));
    }

    #[tokio::test]
    async fn read_unknown_tag_surfaces_typed_error_not_silent_zero() {
        let c = MockController::new(ControllerKind::Plc);
        let err = c.read_tag(&addr("DB1.DBX0.0")).await.unwrap_err();
        assert!(matches!(err, ReadError::UnknownTag(_)));
    }

    #[tokio::test]
    async fn write_to_declared_tag_updates_value_and_returns_result() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MW0"),
            initial: TagValue::Int(0),
        })
        .await
        .unwrap();
        let result = c
            .dispatch(
                ActuatorCommand::WriteTag {
                    address: "%MW0".into(),
                    value: TagValue::Int(42),
                },
                test_permit(),
            )
            .await
            .unwrap();
        assert_eq!(result.command_kind, "write-tag");
        assert_eq!(result.data["type"], "int");
        assert_eq!(result.data["previous_type"], "int");
        // State updated.
        assert_eq!(c.read_tag(&addr("%MW0")).await.unwrap(), TagValue::Int(42));
    }

    #[tokio::test]
    async fn write_to_unknown_tag_surfaces_as_bad_command() {
        let c = MockController::new(ControllerKind::Plc);
        let err = c
            .dispatch(
                ActuatorCommand::WriteTag {
                    address: "missing".into(),
                    value: TagValue::Bool(true),
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn write_with_type_mismatch_surfaces_as_bad_command() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MX0.0"),
            initial: TagValue::Bool(false),
        })
        .await
        .unwrap();
        let err = c
            .dispatch(
                ActuatorCommand::WriteTag {
                    address: "%MX0.0".into(),
                    value: TagValue::Int(1),
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
        // State NOT mutated — the failed write rolls back.
        assert_eq!(
            c.read_tag(&addr("%MX0.0")).await.unwrap(),
            TagValue::Bool(false)
        );
    }

    #[tokio::test]
    async fn malformed_address_in_command_surfaces_as_bad_command() {
        // Empty / overlong / control-char addresses get rejected
        // at dispatch time, before any tag-map lookup.
        let c = MockController::new(ControllerKind::Plc);
        let err = c
            .dispatch(
                ActuatorCommand::WriteTag {
                    address: "".into(),
                    value: TagValue::Bool(true),
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn halt_is_accepted_and_does_not_clear_tags() {
        // Halt mirrors real PLC scan-pause: tags retain values.
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MX0.0"),
            initial: TagValue::Bool(true),
        })
        .await
        .unwrap();
        c.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(c.tag_count().await, 1);
        assert_eq!(
            c.read_tag(&addr("%MX0.0")).await.unwrap(),
            TagValue::Bool(true)
        );
    }

    #[tokio::test]
    async fn non_write_non_halt_commands_are_rejected() {
        // MoveJoint, Capture, Scan, DispatchJob all rejected
        // with BadCommand — controller doesn't speak those
        // command shapes.
        let c = MockController::new(ControllerKind::Plc);
        let err = c
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 0.0,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn dispatched_and_rejected_commands_all_appear_in_received_log() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MW0"),
            initial: TagValue::Int(0),
        })
        .await
        .unwrap();
        c.dispatch(
            ActuatorCommand::WriteTag {
                address: "%MW0".into(),
                value: TagValue::Int(7),
            },
            test_permit(),
        )
        .await
        .unwrap();
        let _ = c
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 80,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await;
        c.dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        let log = c.received().await;
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].kind(), CommandKind::WriteTag);
        assert_eq!(log[1].kind(), CommandKind::Capture);
        assert_eq!(log[2].kind(), CommandKind::Halt);
    }

    #[tokio::test]
    async fn controller_kind_is_returned_verbatim() {
        // Pin the discriminator round-trip for each variant.
        for k in [
            ControllerKind::Plc,
            ControllerKind::Pac,
            ControllerKind::Cnc,
            ControllerKind::Dcs,
        ] {
            let c = MockController::new(k);
            assert_eq!(c.controller_kind(), k);
        }
    }

    #[tokio::test]
    async fn typed_write_helper_surfaces_unknown_tag_via_typed_error() {
        // The typed-write convenience translates BadCommand
        // strings into the WriteError taxonomy so callers that
        // want richer error matching don't have to grep messages.
        let c = MockController::new(ControllerKind::Plc);
        let err = typed_write(&c, addr("nope"), TagValue::Bool(true), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, WriteError::UnknownTag(_)));
    }

    #[tokio::test]
    async fn typed_write_helper_surfaces_type_mismatch_via_typed_error() {
        let c = MockController::new(ControllerKind::Plc);
        c.declare_tag(TagDeclaration {
            address: addr("%MX0.0"),
            initial: TagValue::Bool(false),
        })
        .await
        .unwrap();
        let err = typed_write(&c, addr("%MX0.0"), TagValue::Int(1), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, WriteError::TypeMismatch { .. }));
    }

    #[tokio::test]
    async fn controller_id_distinct_from_actuator_id() {
        let c = MockController::new(ControllerKind::Plc);
        assert_ne!(c.controller_id().0, c.actuator_id().0);
    }
}
