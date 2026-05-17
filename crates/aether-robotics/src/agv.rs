//! AGV / AMR (Automated Guided Vehicle / Autonomous Mobile Robot)
//! — `MockAgv` with waypoint navigation, route lifecycle, and
//! `Halt`-driven preemption.
//!
//! ## Why N=3, not N=6
//! Industrial arms are 6-DOF (xyz + rxryrz); AGVs are planar
//! vehicles with three controllable degrees of freedom: `x`, `y`,
//! and heading (yaw angle about world z). The const-generic
//! `JointAngles<3>` machinery from V5 carries the count through
//! the type system — the AGV's `RobotController::joint_count`
//! returns 3, matching the same trait surface a 6-DOF arm uses
//! with N=6. Exercises the trait at a different const-generic
//! instantiation than V6 — confirms the generic abstraction
//! actually generalises.
//!
//! ## Route lifecycle
//! ```text
//!   Idle ──load_route──► Idle ──dispatch──► Active ──step──► Active …
//!                                                ├──reach last──► Completed
//!                                                └──Halt──► Preempted
//! ```
//!
//! Routes must be loaded into the AGV (out-of-band; the production
//! plan tooling uses Supabase) BEFORE a `DispatchJob` references
//! them. Dispatching an unknown route_id surfaces as
//! `BadCommand` — refusing to silently no-op the trip preserves
//! the "every command produces a definite outcome" property
//! callers rely on.
//!
//! ## Why a `step(dt)` API instead of background motion
//! Real AGVs run a continuous control loop driven by their own
//! clock. The mock makes time explicit so tests can step the AGV
//! along a route deterministically — no flakiness from sleep
//! timing, no race conditions between test assertions and a
//! background tokio task. Production impls (`UrAgv`,
//! `ZebraAgv`, etc.) implement their own time loop on top of the
//! same trait surface.
//!
//! ## What's NOT here yet
//! Discovery integration (extending `ProbeKind` with `Agv`) and
//! the e-stop signal wiring (`EstopSignal` from V8) layer on
//! top in subsequent commits. The Actuator + RobotController
//! contracts pinned here stay unchanged.

use crate::pose::Pose;
use crate::robot::RobotController;
use aether_actuators::actuator::{enforce_permit, Actuator, ActuatorError, ActuatorResult};
use aether_actuators::command::ActuatorCommand;
use aether_actuators::permit::ActuatorPermit;
use aether_core::{ActuatorId, RobotId};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;

/// Three controllable degrees of freedom on a planar AGV: x, y,
/// heading. Matches the `JointAngles<N>` const-generic from V5
/// with N=3 — same trait surface a 6-DOF arm uses with N=6.
pub const AGV_JOINT_COUNT: usize = 3;

/// Tolerance for "reached the waypoint" — 5 cm. Tight enough that
/// real factory AGVs (which lock-pin to docking targets at <1 cm)
/// reach all waypoints; loose enough that a poorly-calibrated
/// odometry estimate still advances rather than spinning at the
/// approach. Tenants override per-AGV via `with_tolerance`.
pub const DEFAULT_WAYPOINT_TOLERANCE_M: f64 = 0.05;

/// One stop along a route. `heading` is optional: when present,
/// the AGV faces that direction on arrival (e.g. docking at a
/// charger); when absent, the AGV keeps whatever heading it
/// had on approach.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Waypoint {
    pub x: f64,
    pub y: f64,
    pub heading: Option<f64>,
}

impl Waypoint {
    pub fn xy(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            heading: None,
        }
    }

    pub fn with_heading(mut self, h: f64) -> Self {
        self.heading = Some(h);
        self
    }
}

/// Route identifier — opaque string from the route-planning layer
/// (Supabase `routes` table in production). Newtype so the type
/// system rejects `MachineId` where a `RouteId` is wanted.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct RouteId(pub String);

impl RouteId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Route {
    pub id: RouteId,
    pub waypoints: Vec<Waypoint>,
    /// Top-speed limit along the route. The AGV interpolates at
    /// this speed (production uses a per-segment max with a
    /// motion-profile that ramps); the mock walks at a constant
    /// `max_speed_mps`.
    pub max_speed_mps: f64,
}

/// Lifecycle state of the AGV's active route. Tagged with `state`
/// for JSON discoverability — Edge Function consumers can filter
/// `state == "active"` without parsing the rest of the payload.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RouteStatus {
    Idle,
    Active {
        route_id: RouteId,
        waypoint_index: usize,
    },
    Completed {
        route_id: RouteId,
    },
    Preempted {
        route_id: RouteId,
        reason: String,
    },
}

#[derive(Clone, Copy, Debug)]
struct AgvPose {
    x: f64,
    y: f64,
    heading: f64,
}

impl AgvPose {
    const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        heading: 0.0,
    };
}

/// 3-DOF (x, y, heading) AGV with route-driven waypoint navigation.
/// Records every dispatched command (mirrors V6's `MockArm` and
/// V2's `MockCamera`).
pub struct MockAgv {
    robot_id: RobotId,
    pose: Mutex<AgvPose>,
    status: Mutex<RouteStatus>,
    /// Pre-loaded routes keyed by id. `DispatchJob` references a
    /// route by id, which must already be present here. Production
    /// loads from Supabase on AGV pairing.
    routes: Mutex<HashMap<RouteId, Route>>,
    waypoint_tolerance_m: f64,
    received: Mutex<Vec<ActuatorCommand>>,
}

impl MockAgv {
    pub fn new() -> Self {
        Self {
            robot_id: RobotId::new(),
            pose: Mutex::new(AgvPose::ZERO),
            status: Mutex::new(RouteStatus::Idle),
            routes: Mutex::new(HashMap::new()),
            waypoint_tolerance_m: DEFAULT_WAYPOINT_TOLERANCE_M,
            received: Mutex::new(Vec::new()),
        }
    }

    /// Builder: override the per-waypoint arrival tolerance.
    pub fn with_tolerance(mut self, meters: f64) -> Self {
        self.waypoint_tolerance_m = meters;
        self
    }

    /// Pre-load a route. Must be called BEFORE a `DispatchJob`
    /// references the route_id. In production this is wired to
    /// the Supabase `routes` table; the AGV's startup handshake
    /// pulls its assigned routes into memory.
    pub async fn load_route(&self, route: Route) {
        self.routes.lock().await.insert(route.id.clone(), route);
    }

    /// Snapshot every received command.
    pub async fn received(&self) -> Vec<ActuatorCommand> {
        self.received.lock().await.clone()
    }

    pub async fn status(&self) -> RouteStatus {
        self.status.lock().await.clone()
    }

    /// Walk the AGV toward its current waypoint by `max_speed_mps *
    /// dt_seconds`. Returns the updated `RouteStatus`. Tests drive
    /// this in a tight loop to deterministically navigate routes
    /// without sleeping.
    ///
    /// Returns `Idle`/`Completed`/`Preempted` unchanged — `step`
    /// only does work when status is `Active`.
    pub async fn step(&self, dt_seconds: f64) -> RouteStatus {
        // Snapshot status first (drop the lock before we take the
        // routes lock to avoid lock-order issues).
        let snapshot = self.status.lock().await.clone();
        let (route_id, waypoint_index) = match snapshot {
            RouteStatus::Active {
                ref route_id,
                waypoint_index,
            } => (route_id.clone(), waypoint_index),
            _ => return snapshot,
        };

        let routes = self.routes.lock().await;
        let route = match routes.get(&route_id) {
            Some(r) => r.clone(),
            None => {
                // Defensive: route was unloaded after dispatch. Mark
                // preempted so callers see the orphan and don't
                // wait forever for progress.
                drop(routes);
                let preempt = RouteStatus::Preempted {
                    route_id,
                    reason: "route unloaded mid-navigation".into(),
                };
                *self.status.lock().await = preempt.clone();
                return preempt;
            }
        };
        drop(routes);

        let target = match route.waypoints.get(waypoint_index) {
            Some(wp) => *wp,
            None => {
                // Shouldn't happen — DispatchJob validates len > 0
                // and the "advance index" branch below transitions
                // to Completed when past the last waypoint.
                // Belt-and-braces.
                let preempt = RouteStatus::Preempted {
                    route_id,
                    reason: format!("waypoint index {waypoint_index} out of bounds"),
                };
                *self.status.lock().await = preempt.clone();
                return preempt;
            }
        };

        // Compute displacement to the target waypoint.
        let mut pose = *self.pose.lock().await;
        let dx = target.x - pose.x;
        let dy = target.y - pose.y;
        let dist = (dx * dx + dy * dy).sqrt();

        // Walk toward the target if not already within tolerance.
        // Heading tracks velocity direction so the AGV "faces" its
        // motion (real AGVs orient their forward axis along the
        // path). Heading update is skipped when we're already on
        // the waypoint to avoid atan2(0, 0) ambiguity.
        if dist > self.waypoint_tolerance_m {
            let max_step = route.max_speed_mps * dt_seconds;
            let step_size = max_step.min(dist);
            pose.x += dx * step_size / dist;
            pose.y += dy * step_size / dist;
            pose.heading = dy.atan2(dx);
            *self.pose.lock().await = pose;

            // Re-check after walking. A step that lands exactly on
            // the waypoint (or within tolerance) MUST trigger
            // advancement on the same call — otherwise a caller
            // would have to step a second time to register arrival,
            // and a "step the AGV to the end of the route" loop
            // would be off-by-one.
            let new_dx = target.x - pose.x;
            let new_dy = target.y - pose.y;
            let new_dist = (new_dx * new_dx + new_dy * new_dy).sqrt();
            if new_dist > self.waypoint_tolerance_m {
                return RouteStatus::Active {
                    route_id,
                    waypoint_index,
                };
            }
        }

        // Arrived: snap to exact target coords, set heading if
        // the waypoint specified one (overrides the velocity-
        // tracking heading from the walk), advance the index.
        pose.x = target.x;
        pose.y = target.y;
        if let Some(h) = target.heading {
            pose.heading = h;
        }
        *self.pose.lock().await = pose;

        let next_index = waypoint_index + 1;
        let new_status = if next_index >= route.waypoints.len() {
            RouteStatus::Completed { route_id }
        } else {
            RouteStatus::Active {
                route_id,
                waypoint_index: next_index,
            }
        };
        *self.status.lock().await = new_status.clone();
        new_status
    }
}

impl Default for MockAgv {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Actuator for MockAgv {
    fn actuator_id(&self) -> ActuatorId {
        ActuatorId::from_uuid(self.robot_id.into_uuid())
    }
    fn actuator_kind(&self) -> &'static str {
        "agv"
    }
    async fn dispatch(
        &self,
        cmd: ActuatorCommand,
        permit: ActuatorPermit,
    ) -> Result<ActuatorResult, ActuatorError> {
        enforce_permit(&permit)?;
        self.received.lock().await.push(cmd.clone());

        match cmd {
            ActuatorCommand::DispatchJob {
                route_id,
                destination,
                priority,
            } => {
                let id = RouteId::new(&route_id);
                let routes = self.routes.lock().await;
                let route = routes.get(&id).cloned().ok_or_else(|| {
                    ActuatorError::BadCommand(format!(
                        "unknown route_id `{route_id}` — load via `load_route` first"
                    ))
                })?;
                drop(routes);
                if route.waypoints.is_empty() {
                    return Err(ActuatorError::BadCommand(format!(
                        "route `{route_id}` has no waypoints"
                    )));
                }
                *self.status.lock().await = RouteStatus::Active {
                    route_id: id.clone(),
                    waypoint_index: 0,
                };
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "dispatch-job".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({
                        "route_id": route_id,
                        "destination": destination,
                        "priority": priority,
                        "waypoint_count": route.waypoints.len(),
                    }),
                })
            }
            ActuatorCommand::Halt => {
                // Preempt the active route (if any). Halt on an
                // Idle/Completed AGV is a no-op but ALWAYS succeeds
                // — universal-preemption contract from V1.
                let mut status = self.status.lock().await;
                if let RouteStatus::Active { route_id, .. } = status.clone() {
                    *status = RouteStatus::Preempted {
                        route_id,
                        reason: "halt-command".into(),
                    };
                }
                Ok(ActuatorResult {
                    actuator_id: self.actuator_id(),
                    command_kind: "halt".into(),
                    completed_at: Utc::now(),
                    data: serde_json::json!({"halted": true}),
                })
            }
            other => Err(ActuatorError::BadCommand(format!(
                "agv does not accept command kind {} (use DispatchJob)",
                other.kind().slug()
            ))),
        }
    }
}

#[async_trait]
impl RobotController for MockAgv {
    fn robot_id(&self) -> RobotId {
        self.robot_id
    }
    fn joint_count(&self) -> usize {
        AGV_JOINT_COUNT
    }
    /// Pose mapping: AGVs are planar, so world z=0. Heading is the
    /// rotation about world z (yaw). rx and ry are always zero.
    async fn current_pose(&self) -> Pose {
        let p = *self.pose.lock().await;
        Pose {
            xyz: [p.x, p.y, 0.0],
            rxryrz: [0.0, 0.0, p.heading],
        }
    }
    async fn current_joints(&self) -> Vec<f64> {
        let p = *self.pose.lock().await;
        vec![p.x, p.y, p.heading]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joint::JointAngles;
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
                code: "agv-operator".into(),
                issued_at: Utc::now() - Duration::days(30),
                expires_at: Some(Utc::now() + Duration::days(30)),
                revoked: false,
            }],
            required_certs: vec!["agv-operator".into()],
            user_lockout_reason: None,
            machine_fault: None,
            as_of: Utc::now(),
        };
        gate(&req).expect("interlock should approve in test fixture")
    }

    fn route(id: &str, waypoints: Vec<Waypoint>) -> Route {
        Route {
            id: RouteId::new(id),
            waypoints,
            max_speed_mps: 1.0,
        }
    }

    fn dispatch_cmd(route_id: &str) -> ActuatorCommand {
        ActuatorCommand::DispatchJob {
            route_id: route_id.into(),
            destination: "test-bay".into(),
            priority: 5,
        }
    }

    #[tokio::test]
    async fn initial_status_is_idle() {
        let agv = MockAgv::new();
        assert!(matches!(agv.status().await, RouteStatus::Idle));
    }

    #[tokio::test]
    async fn dispatch_unknown_route_id_surfaces_bad_command() {
        // Refusing to no-op preserves the "every dispatch produces
        // a definite outcome" property — operator gets a typed
        // error instead of an AGV that mysteriously stays Idle.
        let agv = MockAgv::new();
        let err = agv
            .dispatch(dispatch_cmd("never-loaded"), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(ref m) if m.contains("unknown route_id")));
        // Status MUST stay Idle on rejection.
        assert!(matches!(agv.status().await, RouteStatus::Idle));
    }

    #[tokio::test]
    async fn dispatch_empty_route_surfaces_bad_command() {
        // A loaded-but-empty route is a config bug; surface it
        // rather than silently transitioning to Completed.
        let agv = MockAgv::new();
        agv.load_route(route("empty", vec![])).await;
        let err = agv
            .dispatch(dispatch_cmd("empty"), test_permit())
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(_)));
    }

    #[tokio::test]
    async fn dispatch_known_route_transitions_to_active_at_index_zero() {
        let agv = MockAgv::new();
        agv.load_route(route("r1", vec![Waypoint::xy(1.0, 0.0)]))
            .await;
        agv.dispatch(dispatch_cmd("r1"), test_permit())
            .await
            .unwrap();
        match agv.status().await {
            RouteStatus::Active {
                route_id,
                waypoint_index,
            } => {
                assert_eq!(route_id, RouteId::new("r1"));
                assert_eq!(waypoint_index, 0);
            }
            other => panic!("expected Active, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn step_makes_progress_toward_current_waypoint() {
        // Single-step partial-distance check: AGV starts at origin,
        // target is (10, 0), max_speed=1, dt=1 → AGV moves to
        // (1, 0), still Active (not arrived yet).
        let agv = MockAgv::new();
        agv.load_route(route("long", vec![Waypoint::xy(10.0, 0.0)]))
            .await;
        agv.dispatch(dispatch_cmd("long"), test_permit())
            .await
            .unwrap();

        let status = agv.step(1.0).await;
        let pose = agv.current_pose().await;
        assert!(matches!(status, RouteStatus::Active { .. }));
        assert!((pose.xyz[0] - 1.0).abs() < 1e-9);
        assert!(pose.xyz[1].abs() < 1e-9);
    }

    #[tokio::test]
    async fn reaching_waypoint_within_tolerance_advances_index() {
        // Two-waypoint route. Step far enough to reach (1, 0)
        // exactly, then a second step that won't reach (5, 0)
        // — index should advance to 1 between steps.
        let agv = MockAgv::new();
        agv.load_route(route(
            "two",
            vec![Waypoint::xy(1.0, 0.0), Waypoint::xy(5.0, 0.0)],
        ))
        .await;
        agv.dispatch(dispatch_cmd("two"), test_permit())
            .await
            .unwrap();

        // First step (dt=2.0, max_speed=1.0) → would advance 2m,
        // but distance is 1m so we snap to (1,0), index → 1.
        let s = agv.step(2.0).await;
        match s {
            RouteStatus::Active { waypoint_index, .. } => assert_eq!(waypoint_index, 1),
            other => panic!("expected Active at index 1, got {other:?}"),
        }
        let pose = agv.current_pose().await;
        assert!((pose.xyz[0] - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn reaching_final_waypoint_transitions_to_completed() {
        let agv = MockAgv::new();
        agv.load_route(route("one", vec![Waypoint::xy(1.0, 0.0)]))
            .await;
        agv.dispatch(dispatch_cmd("one"), test_permit())
            .await
            .unwrap();

        let s = agv.step(10.0).await;
        match s {
            RouteStatus::Completed { route_id } => assert_eq!(route_id, RouteId::new("one")),
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn waypoint_with_explicit_heading_sets_pose_heading() {
        // Arriving at a docking waypoint with `heading=Some(π/2)`
        // should snap heading to π/2 (regardless of approach
        // direction). Production AGVs do this to align with a
        // charger or conveyor.
        let agv = MockAgv::new();
        let pi_2 = std::f64::consts::FRAC_PI_2;
        agv.load_route(route(
            "dock",
            vec![Waypoint::xy(1.0, 0.0).with_heading(pi_2)],
        ))
        .await;
        agv.dispatch(dispatch_cmd("dock"), test_permit())
            .await
            .unwrap();
        agv.step(10.0).await;
        let pose = agv.current_pose().await;
        assert!((pose.rxryrz[2] - pi_2).abs() < 1e-9);
    }

    #[tokio::test]
    async fn halt_mid_route_transitions_to_preempted() {
        // Load-bearing safety property: Halt during navigation
        // stops the AGV with a Preempted status that carries the
        // reason. Auditors see "operator halted at waypoint 0"
        // rather than "AGV mysteriously stopped".
        let agv = MockAgv::new();
        agv.load_route(route(
            "long",
            vec![Waypoint::xy(100.0, 0.0), Waypoint::xy(200.0, 0.0)],
        ))
        .await;
        agv.dispatch(dispatch_cmd("long"), test_permit())
            .await
            .unwrap();
        agv.step(1.0).await;

        // Now Halt.
        let result = agv
            .dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        match agv.status().await {
            RouteStatus::Preempted { route_id, reason } => {
                assert_eq!(route_id, RouteId::new("long"));
                assert_eq!(reason, "halt-command");
            }
            other => panic!("expected Preempted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn halt_when_idle_is_a_safe_noop() {
        // A halt on an idle AGV must still succeed — the universal-
        // preemption contract says every actuator MUST honor halt.
        let agv = MockAgv::new();
        let result = agv
            .dispatch(ActuatorCommand::Halt, test_permit())
            .await
            .unwrap();
        assert_eq!(result.command_kind, "halt");
        assert!(matches!(agv.status().await, RouteStatus::Idle));
    }

    #[tokio::test]
    async fn step_after_completion_is_a_noop() {
        // Calling step() on a completed route shouldn't transition
        // anywhere. Idempotent terminal state.
        let agv = MockAgv::new();
        agv.load_route(route("one", vec![Waypoint::xy(1.0, 0.0)]))
            .await;
        agv.dispatch(dispatch_cmd("one"), test_permit())
            .await
            .unwrap();
        agv.step(10.0).await; // completes
        let s = agv.step(10.0).await; // additional step
        assert!(matches!(s, RouteStatus::Completed { .. }));
    }

    #[tokio::test]
    async fn move_joint_rejected_by_agv_with_bad_command() {
        // Arm commands on an AGV are routing bugs. The error
        // message hints at the right command for the operator.
        let agv = MockAgv::new();
        let err = agv
            .dispatch(
                ActuatorCommand::MoveJoint {
                    joint: 0,
                    target_rad: 0.5,
                },
                test_permit(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ActuatorError::BadCommand(ref m) if m.contains("DispatchJob")));
    }

    #[tokio::test]
    async fn capture_rejected_by_agv_with_bad_command() {
        let agv = MockAgv::new();
        let err = agv
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
    async fn robot_controller_reports_n_equals_three() {
        // The N=3 contract for AGVs. Trait-object callers iterate
        // 0..joint_count() and would crash if this lied about the
        // length.
        let agv = MockAgv::new();
        let robot: &dyn RobotController = &agv;
        assert_eq!(robot.joint_count(), AGV_JOINT_COUNT);
        let joints = robot.current_joints().await;
        assert_eq!(joints.len(), AGV_JOINT_COUNT);
    }

    #[tokio::test]
    async fn pose_mapping_planar_z_zero_heading_as_yaw() {
        // After driving to (1, 0) with heading π/2, the Pose
        // should be xyz=[1, 0, 0] (z=0 since AGV is planar) and
        // rxryrz=[0, 0, π/2]. Documenting the AGV→Pose mapping so
        // a future trait-object caller doesn't trip on it.
        let agv = MockAgv::new();
        let pi_2 = std::f64::consts::FRAC_PI_2;
        agv.load_route(route(
            "dock",
            vec![Waypoint::xy(1.0, 0.0).with_heading(pi_2)],
        ))
        .await;
        agv.dispatch(dispatch_cmd("dock"), test_permit())
            .await
            .unwrap();
        agv.step(10.0).await;
        let pose = agv.current_pose().await;
        assert!((pose.xyz[0] - 1.0).abs() < 1e-9);
        assert!(pose.xyz[1].abs() < 1e-9);
        assert!(pose.xyz[2].abs() < 1e-9, "planar AGV has z=0");
        assert!(pose.rxryrz[0].abs() < 1e-9);
        assert!(pose.rxryrz[1].abs() < 1e-9);
        assert!((pose.rxryrz[2] - pi_2).abs() < 1e-9);
    }

    #[tokio::test]
    async fn typed_joint_count_constant_matches_runtime() {
        // The trait surface returns runtime `joint_count() == 3`;
        // the typed `JointAngles<AGV_JOINT_COUNT>::count()` must
        // agree. A drift between them would mean a const-generic
        // user and a trait-object user disagree on what an AGV is.
        assert_eq!(JointAngles::<AGV_JOINT_COUNT>::count(), 3);
        assert_eq!(MockAgv::new().joint_count(), 3);
    }

    #[tokio::test]
    async fn dispatched_and_rejected_commands_all_appear_in_received_log() {
        // Audit trail of every attempt, mirroring V6 MockArm.
        let agv = MockAgv::new();
        let _ = agv.dispatch(dispatch_cmd("unknown"), test_permit()).await; // BadCommand
        let _ = agv.dispatch(ActuatorCommand::Halt, test_permit()).await;
        let log = agv.received().await;
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].kind().slug(), "dispatch-job");
        assert_eq!(log[1].kind().slug(), "halt");
    }

    #[tokio::test]
    async fn route_status_serde_round_trip_for_each_variant() {
        // Wire-format stability for the future actuator_commands
        // table that'll persist this. Internal tag (`state`) keeps
        // the JSON discoverable; pin so a typo can't drift.
        let cases = vec![
            RouteStatus::Idle,
            RouteStatus::Active {
                route_id: RouteId::new("r1"),
                waypoint_index: 2,
            },
            RouteStatus::Completed {
                route_id: RouteId::new("r1"),
            },
            RouteStatus::Preempted {
                route_id: RouteId::new("r1"),
                reason: "halt-command".into(),
            },
        ];
        for c in cases {
            let s = serde_json::to_string(&c).unwrap();
            assert!(s.contains("\"state\""), "missing tag: {s}");
            let back: RouteStatus = serde_json::from_str(&s).unwrap();
            assert_eq!(back, c);
        }
    }
}
