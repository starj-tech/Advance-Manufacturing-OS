//! Robotic arm — `MockArm` 6-DOF controller with simplified
//! forward / inverse kinematics + joint-limit gating on every
//! command dispatch.
//!
//! ## Kinematics model
//! This is a **simplified** arm model used for testing the trait
//! contract end-to-end. The joint→pose mapping is:
//!
//!   * joints[0..=2] → world translation x, y, z (meters)
//!   * joints[3..=5] → world rotation rx, ry, rz (radians)
//!
//! Real industrial arms have COUPLED joints (shoulder rotation
//! moves the elbow in world frame, etc.); their forward kinematics
//! is a chain of Denavit-Hartenberg parameter multiplications.
//! That's the job of a vendor-specific impl in a future session
//! (e.g. `UrArm`, `FanucArm`) — the trait shape stays the same.
//!
//! The simplified model is genuinely useful for two purposes:
//!   1. **Trait contract**: any caller (V8 robot pipeline, future
//!      InspectionPipeline-equivalent) can be tested against this
//!      arm without standing up a vendor SDK.
//!   2. **Property invariants**: `forward(inverse(p)) == p` holds
//!      exactly (modulo Euler singularities), which lets proptest
//!      exercise the trait surface across thousands of random
//!      configs.
//!
//! ## Joint-limit enforcement
//! Every `MoveJoint` and `MoveLinear` dispatch first computes the
//! target joint configuration, then runs it through
//! `JointLimits::check`. A violation surfaces as
//! `ActuatorError::BadCommand` with the violating joint index;
//! the arm state stays at the pre-dispatch configuration. This
//! is the load-bearing safety property of V6 — no command can
//! drive an arm joint past its hardware-defined range.
//!
//! ## What about IK singularities?
//! The simplified model has no singularities — every reachable
//! pose has exactly one joint configuration. Real arms have
//! infinite IK solutions at some poses and zero at others; that's
//! handled by a vendor-specific IK solver in the production impl.

use crate::joint::{JointAngles, JointLimits};
use crate::pose::Pose;
use crate::robot::RobotController;
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, RobotId};
use async_trait::async_trait;
use chrono::Utc;
use std::f64::consts::PI;
use tokio::sync::Mutex;

/// Joint count for the canonical industrial arm (Kuka KR series,
/// ABB IRB, Universal Robots UR-5/UR-10, etc. — all 6-DOF).
pub const ARM_JOINT_COUNT: usize = 6;

/// Default symmetric joint limits — `±2π` per joint. Production
/// arms have tighter and asymmetric limits; tenants override
/// via `MockArm::with_limits`.
pub fn default_arm_limits() -> JointLimits<ARM_JOINT_COUNT> {
    JointLimits::symmetric(2.0 * PI)
}

/// Forward kinematics: joint configuration → tool-frame pose.
/// Pure function; no state. The simplified mapping is documented
/// in the module docs.
pub fn forward_kinematics(joints: &JointAngles<ARM_JOINT_COUNT>) -> Pose {
    Pose {
        xyz: [joints.radians[0], joints.radians[1], joints.radians[2]],
        rxryrz: [joints.radians[3], joints.radians[4], joints.radians[5]],
    }
}

/// Inverse kinematics: tool-frame pose → joint configuration.
/// Pure function; no state. The simplified IK is a direct field
/// projection — see module docs on why this is OK for a mock.
pub fn inverse_kinematics(pose: &Pose) -> JointAngles<ARM_JOINT_COUNT> {
    JointAngles::new([
        pose.xyz[0],
        pose.xyz[1],
        pose.xyz[2],
        pose.rxryrz[0],
        pose.rxryrz[1],
        pose.rxryrz[2],
    ])
}

/// 6-DOF mock arm controller. Records every command for test
/// assertion (mirrors V2's MockCamera pattern). Joint state is
/// mutex-protected so concurrent dispatch from multiple pipelines
/// stays consistent.
pub struct MockArm {
    robot_id: RobotId,
    joints: Mutex<JointAngles<ARM_JOINT_COUNT>>,
    limits: JointLimits<ARM_JOINT_COUNT>,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockArm {
    pub fn new() -> Self {
        Self {
            robot_id: RobotId::new(),
            joints: Mutex::new(JointAngles::zero()),
            limits: default_arm_limits(),
            received: Mutex::new(Vec::new()),
        }
    }

    /// Builder: override joint limits. Tenants with vendor-specific
    /// limits (UR-5 has ±2π but Kuka KR-3 has narrower wrist
    /// joints) supply their data-sheet values here.
    pub fn with_limits(mut self, limits: JointLimits<ARM_JOINT_COUNT>) -> Self {
        self.limits = limits;
        self
    }

    /// Builder: seed the initial joint configuration. Production
    /// arms boot at a home position; tests use this to start at a
    /// specific configuration that exercises a path.
    pub fn with_joints(self, joints: JointAngles<ARM_JOINT_COUNT>) -> Self {
        // Set under the lock — Self::new already initialized to
        // zero, so we just overwrite.
        if let Ok(mut g) = self.joints.try_lock() {
            *g = joints;
        }
        self
    }

    /// Snapshot every command the arm has seen so far. Mirrors
    /// MockCamera::received for cross-crate consistency.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }

    /// Typed-view accessor — the trait surface only exposes
    /// `Vec<f64>` (per `RobotController` contract). This is for
    /// tests / specific-type consumers that want the const-generic
    /// flavor.
    pub async fn current_joints_typed(&self) -> JointAngles<ARM_JOINT_COUNT> {
        *self.joints.lock().await
    }
}

impl Default for MockArm {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Actuator for MockArm {
    fn actuator_id(&self) -> ActuatorId {
        ActuatorId::from_uuid(self.robot_id.into_uuid())
    }
    fn actuator_kind(&self) -> &'static str {
        "robot-arm"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        // Record every received command BEFORE the kind dispatch —
        // even commands we reject get logged so an auditor can
        // see what was attempted.
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::MoveJoint { joint, target_rad } => {
                let idx = joint as usize;
                if idx >= ARM_JOINT_COUNT {
                    return Err(ActuatorError::BadCommand(format!(
                        "joint index {idx} out of range 0..={}",
                        ARM_JOINT_COUNT - 1
                    )));
                }
                // Build the would-be configuration, check limits.
                // The check runs against the FULL config — even if
                // only one joint changed, a degenerate-config bug
                // would surface here.
                let mut proposed = *self.joints.lock().await;
                proposed.radians[idx] = target_rad;
                if let Err(e) = self.limits.check(&proposed) {
                    return Err(ActuatorError::BadCommand(format!("joint limit: {e}")));
                }
                *self.joints.lock().await = proposed;
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "move-joint".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({
                        "joint": idx,
                        "target_rad": target_rad,
                        "achieved_rad": target_rad,
                    }),
                })
            }
            ActuatorCommand::MoveLinear {
                xyz,
                rxryrz,
                speed_mps,
            } => {
                let target_pose = Pose { xyz, rxryrz };
                let target_joints = inverse_kinematics(&target_pose);
                if let Err(e) = self.limits.check(&target_joints) {
                    return Err(ActuatorError::BadCommand(format!("joint limit: {e}")));
                }
                *self.joints.lock().await = target_joints;
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "move-linear".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({
                        "xyz": xyz,
                        "rxryrz": rxryrz,
                        "speed_mps": speed_mps,
                    }),
                })
            }
            ActuatorCommand::Halt => {
                // Halt acknowledges without state change. Real
                // arms send a power-off signal to the servos here;
                // the mock just records.
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "arm does not accept command kind {}",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl RobotController for MockArm {
    fn robot_id(&self) -> RobotId {
        self.robot_id
    }
    fn joint_count(&self) -> usize {
        ARM_JOINT_COUNT
    }
    async fn current_pose(&self) -> Pose {
        forward_kinematics(&*self.joints.lock().await)
    }
    async fn current_joints(&self) -> Vec<f64> {
        self.joints.lock().await.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_actuators::gate::gate;
    use aether_safety::interlock::{Certification, UnlockRequest};
    use chrono::Duration;
    use proptest::prelude::*;
    use uuid::Uuid;

    const EPS: f64 = 1e-9;

    /// Production-path permit for tests. `ActuatorPermit::new_unchecked`
    /// is `pub(crate)` to `aether-actuators` — external code (including
    /// these tests) mints through `gate()`.
    fn test_permit() -> ActuatorPermit {
        let req = UnlockRequest {
            user_id: Uuid::nil(),
            machine_id: Uuid::nil(),
            user_certs: vec![Certification {
                user_id: Uuid::nil(),
                code: "arm-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["arm-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock should approve in test fixture")
    }

    fn pose_close(a: &Pose, b: &Pose, eps: f64) -> bool {
        a.xyz
            .iter()
            .zip(b.xyz.iter())
            .all(|(x, y)| (x - y).abs() < eps)
            && a.rxryrz
                .iter()
                .zip(b.rxryrz.iter())
                .all(|(x, y)| (x - y).abs() < eps)
    }

    fn joints_close(
        a: &JointAngles<ARM_JOINT_COUNT>,
        b: &JointAngles<ARM_JOINT_COUNT>,
        eps: f64,
    ) -> bool {
        a.radians
            .iter()
            .zip(b.radians.iter())
            .all(|(x, y)| (x - y).abs() < eps)
    }

    #[test]
    fn forward_of_zero_joints_is_identity() {
        // Home position → identity pose. Production arms calibrate
        // to home at boot, so this is the value telemetry should
        // report immediately after a power cycle.
        let pose = forward_kinematics(&JointAngles::zero());
        assert_eq!(pose, Pose::identity());
    }

    #[test]
    fn forward_inverse_round_trip_for_specific_pose() {
        // Sanity check before the property test: a known pose
        // round-trips exactly.
        let p = Pose {
            xyz: [1.0, -0.5, 0.3],
            rxryrz: [0.1, -0.2, 0.3],
        };
        let j = inverse_kinematics(&p);
        let back = forward_kinematics(&j);
        assert!(pose_close(&p, &back, EPS));
    }

    #[test]
    fn inverse_forward_round_trip_for_specific_joints() {
        let j = JointAngles::<ARM_JOINT_COUNT>::new([0.5, -0.3, 1.2, 0.1, -0.4, 0.2]);
        let pose = forward_kinematics(&j);
        let back = inverse_kinematics(&pose);
        assert!(joints_close(&j, &back, EPS));
    }

    proptest! {
        /// The headline invariant: forward(inverse(p)) ≈ p for any
        /// pose in the reachable workspace. The simplified mock
        /// model has no singularities, so this holds with very
        /// tight tolerance.
        #[test]
        fn forward_inverse_round_trips_for_arbitrary_pose(
            x in -10.0f64..10.0,
            y in -10.0f64..10.0,
            z in -10.0f64..10.0,
            rx in -PI..PI,
            ry in -PI/2.0 + 0.1..PI/2.0 - 0.1,  // avoid gimbal lock
            rz in -PI..PI,
        ) {
            let p = Pose {
                xyz: [x, y, z],
                rxryrz: [rx, ry, rz],
            };
            let j = inverse_kinematics(&p);
            let back = forward_kinematics(&j);
            prop_assert!(
                pose_close(&p, &back, 1e-6),
                "round-trip drift: {:?} → {:?}", p, back
            );
        }

        /// Symmetric invariant: inverse(forward(j)) ≈ j for any
        /// valid joint configuration. Within the joint limits the
        /// simplified IK is a one-to-one map.
        #[test]
        fn inverse_forward_round_trips_for_arbitrary_joints(
            j0 in -PI..PI,
            j1 in -PI..PI,
            j2 in -PI..PI,
            j3 in -PI..PI,
            j4 in -PI/2.0 + 0.1..PI/2.0 - 0.1,  // avoid Euler singularity
            j5 in -PI..PI,
        ) {
            let j = JointAngles::<ARM_JOINT_COUNT>::new([j0, j1, j2, j3, j4, j5]);
            let pose = forward_kinematics(&j);
            let back = inverse_kinematics(&pose);
            prop_assert!(joints_close(&j, &back, 1e-6));
        }
    }

    #[tokio::test]
    async fn move_joint_within_limits_updates_state() {
        let arm = MockArm::new();
        let permit = test_permit();
        arm.dispatch(
            ActuatorCommand::MoveJoint {
                joint: 2,
                target_rad: 0.5,
            },
            permit,
        )
        .await
        .unwrap();
        let joints = arm.current_joints().await;
        assert!((joints[2] - 0.5).abs() < EPS);
    }

    #[tokio::test]
    async fn move_joint_out_of_range_index_surfaces_bad_command() {
        // Joint index 6 (or higher) is invalid for a 6-DOF arm.
        // Without the explicit bounds check this would panic via
        // array indexing — surface a typed error instead.
        let arm = MockArm::new();
        let err = arm
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 7,
                    target_rad: 0.1,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(ref m) if m.contains("joint index")));
    }

    #[tokio::test]
    async fn move_joint_past_limit_leaves_state_unchanged() {
        // Load-bearing safety property: a limit-exceeding move
        // MUST NOT update the joint state. Otherwise the arm would
        // briefly land in an out-of-range config before any human
        // could intervene.
        let arm = MockArm::new().with_limits(JointLimits::symmetric(1.0));
        let initial = arm.current_joints_typed().await;
        let err = arm
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 5.0,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
        let after = arm.current_joints_typed().await;
        assert!(joints_close(&initial, &after, EPS));
    }

    #[tokio::test]
    async fn move_linear_within_limits_lands_at_inverse_of_target() {
        // MoveLinear computes inverse(target), checks limits,
        // updates state. After dispatch, current_pose() should
        // match the target (within EPS).
        let arm = MockArm::new();
        let target = Pose {
            xyz: [0.5, -0.3, 0.2],
            rxryrz: [0.1, 0.05, -0.1],
        };
        arm.dispatch(
            ActuatorCommand::MoveLinear {
                xyz: target.xyz,
                rxryrz: target.rxryrz,
                speed_mps: 0.1,
            },
            test_permit(),
        )
        .await
        .unwrap();
        let achieved = arm.current_pose().await;
        assert!(pose_close(&target, &achieved, EPS));
    }

    #[tokio::test]
    async fn move_linear_past_limits_leaves_state_unchanged() {
        // Tight limits + a target that requires going past them →
        // BadCommand, no state mutation. The "no half-execution"
        // promise from the module docs.
        let arm = MockArm::new().with_limits(JointLimits::symmetric(0.5));
        let initial = arm.current_pose().await;
        let err = arm
            .dispatch(
                ActuatorCommand::MoveLinear {
                    xyz: [2.0, 0.0, 0.0],
                    rxryrz: [0.0, 0.0, 0.0],
                    speed_mps: 0.1,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
        let after = arm.current_pose().await;
        assert!(pose_close(&initial, &after, EPS));
    }

    #[tokio::test]
    async fn halt_is_accepted_universally() {
        // Per V1's universal-preemption contract. A robot that
        // rejected halt would break the e-stop story (V8).
        let arm = MockArm::new();
        let result = arm
            .dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        assert_eq!(result.data["halted"], true);
    }

    #[tokio::test]
    async fn capture_rejected_by_arm_with_bad_command() {
        // A vision command on a robot is a routing bug; surface
        // loudly, don't silently ignore. Mirrors the V2 Camera
        // rejecting MoveJoint.
        let arm = MockArm::new();
        let err = arm
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 85,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn rejected_commands_still_appear_in_received_log() {
        // Audit trail of EVERY attempted command, not just
        // successful ones. Auditors want to see "the operator
        // tried Capture on the arm" — that's a useful training
        // signal.
        let arm = MockArm::new();
        let _ = arm
            .dispatch(
                ActuatorCommand::Capture {
                    quality: 85,
                    format: "png".into(),
                },
                test_permit(),
            )
            .await;
        let log = arm.received().await;
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].kind().slug(), "capture");
    }

    #[tokio::test]
    async fn dyn_robot_controller_dispatch_works() {
        // Object-safety smoke through dispatch path. A `&dyn
        // RobotController` lets the future robot-pipeline drive
        // any concrete arm.
        let arm = MockArm::new();
        let robot: &dyn RobotController = &arm;
        // RobotController is also an Actuator — dispatch goes
        // through the upcast.
        let result = robot
            .dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        assert_eq!(robot.joint_count(), 6);
    }

    #[tokio::test]
    async fn current_pose_reflects_forward_kinematics_of_current_joints() {
        // The trait's current_pose() must agree with
        // forward_kinematics(current_joints) — otherwise the
        // pipeline would see "where the arm thinks it is" diverge
        // from "where the math says the arm is".
        let arm = MockArm::new();
        let initial_joints = JointAngles::<ARM_JOINT_COUNT>::new([0.3, -0.4, 0.5, 0.1, 0.2, -0.1]);
        arm.dispatch(
            ActuatorCommand::MoveLinear {
                xyz: [
                    initial_joints.radians[0],
                    initial_joints.radians[1],
                    initial_joints.radians[2],
                ],
                rxryrz: [
                    initial_joints.radians[3],
                    initial_joints.radians[4],
                    initial_joints.radians[5],
                ],
                speed_mps: 0.1,
            },
            test_permit(),
        )
        .await
        .unwrap();

        let pose_from_trait = arm.current_pose().await;
        let pose_from_fk = forward_kinematics(&arm.current_joints_typed().await);
        assert_eq!(pose_from_trait, pose_from_fk);
    }
}
