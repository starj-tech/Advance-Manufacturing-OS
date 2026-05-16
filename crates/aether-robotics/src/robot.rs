//! [`RobotController`] — `Actuator` supertrait for any controllable
//! robot (arms in V6, AGV/AMR in V7).
//!
//! ## Why a supertrait
//! Every robot IS an actuator (it accepts `MoveJoint`, `MoveLinear`,
//! `DispatchJob`, `Halt`), plus needs vision-pipeline-style state
//! accessors: current pose, current joints, joint count. Modelling
//! these as a supertrait lets a `&dyn RobotController` use both
//! surfaces without casting — same shape as V2's `Camera: Actuator`.
//!
//! ## Why `Vec<f64>` at the trait boundary
//! Concrete impls hold typed `JointAngles<N>` internally for the
//! compile-time count check (see `joint.rs` module docs). The
//! trait surface flattens to `Vec<f64>` so the trait stays
//! object-safe — `&dyn RobotController` is what the future
//! "RobotPipeline" (V8 with e-stop wiring) will dispatch through.
//!
//! Projection lossy? No — joint count is the same on both sides;
//! callers that need the typed view downcast to the concrete impl.

use crate::pose::Pose;
use aether_actuators::actuator::Actuator;
use aether_core::RobotId;
use async_trait::async_trait;

#[async_trait]
pub trait RobotController: Actuator {
    /// Stable robot identity. Audit rows use this AND the
    /// inherited `ActuatorId` (linked via shared UUID — same
    /// pattern as V2's `Camera`).
    fn robot_id(&self) -> RobotId;

    /// Number of joints. Concrete impls' const-generic `N` shows
    /// up here at runtime; trait-object callers branch on this to
    /// size joint loops.
    fn joint_count(&self) -> usize;

    /// Current end-effector / tool-frame pose in world coordinates.
    /// Implementations either read from the controller's feedback
    /// stream (real arm) or compute via forward kinematics (mock
    /// or simulator). Returns the latest-known value; callers
    /// concerned about staleness should consult the underlying
    /// telemetry stream.
    async fn current_pose(&self) -> Pose;

    /// Current joint configuration as a `Vec<f64>` of radians,
    /// length == `joint_count()`. See module docs on why this
    /// isn't `JointAngles<N>`.
    async fn current_joints(&self) -> Vec<f64>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JointAngles;
    use aether_actuators::actuator::{ActuatorError, ActuatorResult};
    use aether_actuators::command::ActuatorCommand;
    use aether_actuators::permit::ActuatorPermit;
    use aether_core::{ActuatorId, RobotId};
    use chrono::Utc;
    use tokio::sync::Mutex;

    /// Minimal `RobotController` for trait-surface tests. No
    /// kinematics — just state recording. The real arm impl
    /// (with forward kinematics + joint-limit enforcement) lands
    /// in V6. The point of this stub is documenting the trait
    /// contract: every method has a callable impl, and a
    /// `&dyn RobotController` works as expected.
    struct StubController {
        id: RobotId,
        joints: Mutex<Vec<f64>>,
        pose: Mutex<Pose>,
    }

    impl StubController {
        fn new() -> Self {
            Self {
                id: RobotId::new(),
                joints: Mutex::new(vec![0.0; 6]),
                pose: Mutex::new(Pose::identity()),
            }
        }
    }

    #[async_trait]
    impl Actuator for StubController {
        fn actuator_id(&self) -> ActuatorId {
            // Same UUID space as RobotId — linkability property
            // from V2 camera carries over.
            ActuatorId::from_uuid(self.id.into_uuid())
        }
        fn actuator_kind(&self) -> &'static str {
            "robot-stub"
        }
        async fn dispatch(
            &self,
            _cmd: ActuatorCommand,
            _permit: ActuatorPermit,
        ) -> Result<ActuatorResult, ActuatorError> {
            // Trait test doesn't exercise dispatch — V6 covers it.
            Ok(ActuatorResult {
                actuator_id: self.actuator_id(),
                command_kind: "stub".into(),
                completed_at: Utc::now(),
                data: serde_json::json!({}),
            })
        }
    }

    #[async_trait]
    impl RobotController for StubController {
        fn robot_id(&self) -> RobotId {
            self.id
        }
        fn joint_count(&self) -> usize {
            6
        }
        async fn current_pose(&self) -> Pose {
            *self.pose.lock().await
        }
        async fn current_joints(&self) -> Vec<f64> {
            self.joints.lock().await.clone()
        }
    }

    #[tokio::test]
    async fn dyn_robot_controller_is_object_safe() {
        // Compile-time proof the trait can live behind `&dyn`. If
        // a future change accidentally added a non-object-safe
        // method (generic param, `Self` return), this would fail
        // to compile.
        let stub = StubController::new();
        let robot: &dyn RobotController = &stub;
        assert_eq!(robot.joint_count(), 6);
        assert_eq!(robot.current_joints().await.len(), 6);
    }

    #[tokio::test]
    async fn robot_id_and_actuator_id_share_underlying_uuid() {
        // Same linkability property as V2's Camera — audit row
        // references both ways without a join table.
        let stub = StubController::new();
        assert_eq!(stub.robot_id().into_uuid(), stub.actuator_id().into_uuid());
    }

    #[tokio::test]
    async fn current_pose_returns_identity_for_fresh_controller() {
        let stub = StubController::new();
        let p = stub.current_pose().await;
        assert_eq!(p, Pose::identity());
    }

    #[tokio::test]
    async fn current_joints_length_matches_joint_count() {
        // Load-bearing for callers that iterate `0..joint_count()`
        // and index into `current_joints()`. A length mismatch
        // would surface as a panic at the first read.
        let stub = StubController::new();
        let n = stub.joint_count();
        let joints = stub.current_joints().await;
        assert_eq!(joints.len(), n);
    }

    #[test]
    fn joint_angles_to_vec_matches_joint_count() {
        // Smoke for the trait-boundary projection used by concrete
        // impls in V6/V7.
        let j: JointAngles<6> = JointAngles::new([0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
        let v = j.to_vec();
        assert_eq!(v.len(), 6);
        assert_eq!(v[3], 0.4);
    }
}
