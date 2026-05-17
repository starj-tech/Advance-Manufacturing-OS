//! Robotics integration — robotic arms + AGV/AMR as bidirectional
//! [`aether_actuators::Actuator`]s.
//!
//! ## What V5 ships (this crate's first commit)
//!
//!   * [`pose::Pose`] — 6-DOF rigid-body pose (xyz + Euler rxryrz).
//!     Composition + inversion via `nalgebra::Isometry3<f64>` so the
//!     math is correct without re-deriving SE(3) algebra here.
//!
//!   * [`joint::JointAngles<N>`] + [`joint::JointLimits<N>`] —
//!     const-generic joint configuration with per-joint range
//!     checks. `N=6` is the typical industrial arm; `N=3` covers
//!     AGV/AMR (`x`, `y`, `heading`). The compile-time joint count
//!     eliminates "off-by-one in joint index" bugs the moment they
//!     happen.
//!
//!   * [`robot::RobotController`] — `Actuator` supertrait that adds
//!     `robot_id`, `joint_count`, `current_pose`, `current_joints`.
//!     Object-safe: returns `Vec<f64>` for joints rather than the
//!     const-generic `JointAngles<N>` (which can't live in a trait
//!     object). Specific impls (`ArmController<N=6>` in V6,
//!     `AgvController<N=3>` in V7) use the typed flavor internally
//!     and project to `Vec<f64>` at the trait boundary.
//!
//! ## What lands later
//!   * V6 — `arm/` module with forward kinematics, joint-limit
//!     gating on `MoveJoint`/`MoveLinear`, `MockArm` for tests.
//!   * V7 — `agv/` module with waypoint navigation + `Route` /
//!     `RouteStatus`.
//!   * V8 — `estop` module wires `EstopSignal` (tokio watch
//!     channel) into the Actuator dispatch path for universal
//!     preemption. `dispatch_with_estop(actuator, cmd, permit,
//!     signal)` is the production entrypoint — it refuses up-front
//!     if already tripped, otherwise races the dispatch against the
//!     trip future via `tokio::select! { biased; ... }` so the stop
//!     wins ties. Manual clear only (ISO 13849 / IEC 62061).
//!
//! ## Why nalgebra and not a hand-rolled SE(3)
//! Two minutes of `Isometry3::from_parts(translation, rotation)`
//! is correct. Twenty hours of "I'll just multiply the rotation
//! matrices by hand" is full of sign-flip and Euler-singularity
//! bugs that surface six months in when a customer's wrist crosses
//! a gimbal lock. Use the library that knows.

pub mod agv;
pub mod arm;
pub mod estop;
pub mod joint;
pub mod pose;
pub mod robot;

pub use agv::{
    MockAgv, Route, RouteId, RouteStatus, Waypoint, AGV_JOINT_COUNT, DEFAULT_WAYPOINT_TOLERANCE_M,
};
pub use arm::{
    default_arm_limits, forward_kinematics, inverse_kinematics, MockArm, ARM_JOINT_COUNT,
};
pub use estop::{dispatch_with_estop, EstopReceiver, EstopSignal};
pub use joint::{JointAngles, JointLimitError, JointLimits};
pub use pose::Pose;
pub use robot::RobotController;
