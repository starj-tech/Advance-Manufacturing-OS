//! 6-DOF rigid-body pose with SE(3) composition.
//!
//! ## Representation
//! Position as `[x, y, z]` in meters. Orientation as
//! `[rx, ry, rz]` — XYZ-intrinsic Euler angles in radians. We
//! pick Euler-XYZ specifically because:
//!
//!   * Most industrial-arm vendors quote tool-frame orientations
//!     this way in their pendant UIs. An operator typing `rx=0,
//!     ry=π/2, rz=0` and seeing the wrist rotate 90° about the
//!     `y` axis matches the printed manual.
//!   * Round-trip-stable through nalgebra's `UnitQuaternion::from_
//!     euler_angles` and back via `euler_angles()`. (Quaternion is
//!     the internal representation we compose with — Euler is just
//!     the wire/UI format.)
//!
//! ## Why not store quaternions directly
//! Three reasons. (1) Wire format: tenants read/write pose JSON in
//! UIs and config files; Euler reads as three numbers, quaternion
//! reads as four numbers plus a "what's this 0.707 doing here?"
//! confusion. (2) Backward compatibility with vendor manuals.
//! (3) Round-trip stability is good enough for non-singular
//! configurations — we only convert at the boundary, composition
//! happens in nalgebra's quaternion-backed Isometry3.
//!
//! ## Singularities
//! Euler-XYZ has a singularity at `ry = ±π/2` (gimbal lock). Pose
//! arithmetic still works correctly because composition routes
//! through quaternion form; only the back-conversion to Euler is
//! ambiguous. The `MoveLinear` command path (V6) avoids this by
//! interpolating in quaternion space, not Euler.

use nalgebra::{Isometry3, Translation3, UnitQuaternion};
use serde::{Deserialize, Serialize};

/// 6-DOF pose: position (m) + XYZ-intrinsic Euler rotation (rad).
/// `Copy` because it's 6 floats — passes by value cleanly through
/// kinematics chains.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Pose {
    pub xyz: [f64; 3],
    pub rxryrz: [f64; 3],
}

impl Pose {
    /// Origin + zero rotation. The identity element of SE(3) under
    /// composition: `p.compose(&Pose::identity()) == p`.
    pub fn identity() -> Self {
        Self {
            xyz: [0.0; 3],
            rxryrz: [0.0; 3],
        }
    }

    /// Pure translation. Convenience for "robot is at (x, y, z)
    /// with no rotation".
    pub fn from_xyz(x: f64, y: f64, z: f64) -> Self {
        Self {
            xyz: [x, y, z],
            rxryrz: [0.0; 3],
        }
    }

    /// Internal: convert to nalgebra's `Isometry3<f64>` — the
    /// quaternion-backed SE(3) element we compose with. Pulled out
    /// so `compose` and `inverse` share one conversion site.
    pub(crate) fn to_isometry(self) -> Isometry3<f64> {
        let t = Translation3::new(self.xyz[0], self.xyz[1], self.xyz[2]);
        let r = UnitQuaternion::from_euler_angles(self.rxryrz[0], self.rxryrz[1], self.rxryrz[2]);
        Isometry3::from_parts(t, r)
    }

    pub(crate) fn from_isometry(iso: Isometry3<f64>) -> Self {
        let t = iso.translation.vector;
        let (rx, ry, rz) = iso.rotation.euler_angles();
        Self {
            xyz: [t.x, t.y, t.z],
            rxryrz: [rx, ry, rz],
        }
    }

    /// SE(3) composition: apply `self` first, then `other`. Reads
    /// left-to-right like function composition (`a.compose(b)` ==
    /// `b ∘ a` in standard math notation). The convention matches
    /// "world frame → base frame → tool frame" forward kinematics:
    /// `base.compose(&tool_offset)` gives the tool's world pose.
    pub fn compose(&self, other: &Pose) -> Pose {
        Pose::from_isometry(self.to_isometry() * other.to_isometry())
    }

    /// SE(3) inverse — used by relative-pose math (`world_from_a.
    /// inverse().compose(&world_from_b)` = `a_from_b`).
    pub fn inverse(&self) -> Pose {
        Pose::from_isometry(self.to_isometry().inverse())
    }
}

impl Default for Pose {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const EPS: f64 = 1e-9;

    fn pose_close(a: &Pose, b: &Pose) -> bool {
        a.xyz
            .iter()
            .zip(b.xyz.iter())
            .all(|(x, y)| (x - y).abs() < EPS)
            && a.rxryrz
                .iter()
                .zip(b.rxryrz.iter())
                .all(|(x, y)| (x - y).abs() < EPS)
    }

    #[test]
    fn identity_round_trips_through_isometry() {
        // Smoke that the Pose ↔ Isometry conversion has no offset
        // bias. A drifted identity here would silently break every
        // kinematic chain that starts from base = identity.
        let id = Pose::identity();
        let back = Pose::from_isometry(id.to_isometry());
        assert!(pose_close(&id, &back));
    }

    #[test]
    fn from_xyz_has_zero_rotation() {
        let p = Pose::from_xyz(1.0, 2.0, 3.0);
        assert_eq!(p.xyz, [1.0, 2.0, 3.0]);
        assert_eq!(p.rxryrz, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn compose_with_identity_is_identity_on_either_side() {
        // Identity element law: p ∘ I = I ∘ p = p. Critical for
        // forward-kinematics base cases.
        let p = Pose {
            xyz: [1.0, -2.0, 3.5],
            rxryrz: [0.1, -0.2, 0.3],
        };
        let id = Pose::identity();
        assert!(pose_close(&p.compose(&id), &p));
        assert!(pose_close(&id.compose(&p), &p));
    }

    #[test]
    fn compose_inverse_yields_identity() {
        // The inverse is a left- AND right-inverse:
        //   p ∘ p⁻¹ = I  and  p⁻¹ ∘ p = I
        let p = Pose {
            xyz: [0.5, 0.7, -1.2],
            rxryrz: [0.1, 0.2, -0.3],
        };
        let inv = p.inverse();
        let id = Pose::identity();
        assert!(pose_close(&p.compose(&inv), &id));
        assert!(pose_close(&inv.compose(&p), &id));
    }

    #[test]
    fn pure_translation_composes_additively() {
        // Two pure translations with no rotation: composition is
        // vector addition. Easy property to check, catches a sign
        // flip in to_isometry/from_isometry if it ever happens.
        let a = Pose::from_xyz(1.0, 0.0, 0.0);
        let b = Pose::from_xyz(0.0, 2.0, 0.0);
        let c = a.compose(&b);
        assert!(pose_close(&c, &Pose::from_xyz(1.0, 2.0, 0.0)));
    }

    #[test]
    fn rotation_then_translation_orders_correctly() {
        // Rotate 90° about z, then translate (1, 0, 0). In the
        // POST-rotation frame the (1, 0, 0) translation should land
        // at world (0, 1, 0) — the rotation reoriented the local x
        // axis onto the world y.
        let rotate = Pose {
            xyz: [0.0; 3],
            rxryrz: [0.0, 0.0, FRAC_PI_2],
        };
        let translate = Pose::from_xyz(1.0, 0.0, 0.0);
        let composed = rotate.compose(&translate);
        // World position should be (0, 1, 0); rotation preserved
        // from `rotate`.
        assert!((composed.xyz[0]).abs() < EPS);
        assert!((composed.xyz[1] - 1.0).abs() < EPS);
        assert!(composed.xyz[2].abs() < EPS);
    }

    #[test]
    fn round_trip_through_isometry_preserves_non_singular_pose() {
        // The standard contract: Pose → Isometry → Pose ≈ original
        // for any pose that isn't at a gimbal-lock singularity.
        let p = Pose {
            xyz: [0.3, -0.4, 0.5],
            rxryrz: [0.1, 0.4, -0.2],
        };
        let back = Pose::from_isometry(p.to_isometry());
        assert!(pose_close(&p, &back), "{:?} vs {:?}", p, back);
    }

    #[test]
    fn serde_round_trip_preserves_pose_exactly() {
        // JSON ↔ Pose for the UI / actuator_commands audit path.
        let p = Pose {
            xyz: [1.0, 2.0, 3.0],
            rxryrz: [0.1, 0.2, 0.3],
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Pose = serde_json::from_str(&s).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn default_is_identity() {
        // Tests sometimes rely on Pose::default() — pin it to
        // identity so a typo doesn't introduce a translated default.
        assert_eq!(Pose::default(), Pose::identity());
    }

    #[test]
    fn associativity_of_compose() {
        // (a ∘ b) ∘ c = a ∘ (b ∘ c). Required for forward-
        // kinematics chains where the parenthesization of joint
        // products is arbitrary.
        let a = Pose {
            xyz: [0.1, 0.2, 0.3],
            rxryrz: [0.05, 0.1, 0.15],
        };
        let b = Pose {
            xyz: [-0.2, 0.4, -0.1],
            rxryrz: [-0.1, 0.05, 0.2],
        };
        let c = Pose {
            xyz: [0.3, -0.1, 0.0],
            rxryrz: [0.2, -0.15, 0.05],
        };
        let left = a.compose(&b).compose(&c);
        let right = a.compose(&b.compose(&c));
        assert!(pose_close(&left, &right));
    }
}
