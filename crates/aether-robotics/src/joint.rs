//! Const-generic joint configuration + range checking.
//!
//! ## Why const generics
//! Industrial arms have 4, 6, or 7 joints depending on vendor; AGVs
//! have 3 (x, y, heading). Encoding the count in the type
//! (`JointAngles<6>` for a Kuka KR series, `JointAngles<3>` for an
//! AMR) eliminates a whole class of off-by-one bugs at compile
//! time — passing a 6-joint angle vector to a 3-joint controller is
//! a type error, not a runtime panic.
//!
//! ## What's NOT const-generic
//! The [`crate::RobotController`] trait — it has to live in a
//! `&dyn` trait object so the V4 pipeline-equivalent (a future
//! "RobotPipeline") can dispatch polymorphically. Const generics
//! and trait objects mix poorly. So the trait surface uses
//! `Vec<f64>`; concrete impls (`ArmController<6>` in V6) hold the
//! typed `JointAngles<N>` internally and project to `Vec<f64>` at
//! the trait boundary.
//!
//! ## Limit checking semantics
//! [`JointLimits::check`] walks every joint and returns the FIRST
//! out-of-range index. We deliberately don't accumulate all
//! violations — the safety story is "any one joint out of range
//! aborts the move", and a single-error response keeps the UI
//! message focused.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Joint configuration for an `N`-jointed manipulator. Stored as
/// radians; degree↔radian conversions live at the wire / UI
/// boundary so the math layer always speaks one unit.
///
/// `Serialize`/`Deserialize` are hand-rolled because serde's
/// derive doesn't cover const-generic arrays — the built-in impls
/// only cover specific fixed N up to 32 and don't unify under
/// `[f64; N]`. We serialize as a JSON array of length N; deserialize
/// length-checks against the const generic and rejects mismatches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointAngles<const N: usize> {
    pub radians: [f64; N],
}

impl<const N: usize> Serialize for JointAngles<N> {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.radians.as_slice().serialize(ser)
    }
}

impl<'de, const N: usize> Deserialize<'de> for JointAngles<N> {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let v = <Vec<f64>>::deserialize(de)?;
        if v.len() != N {
            return Err(serde::de::Error::invalid_length(
                v.len(),
                &format!("array of length {N}").as_str(),
            ));
        }
        let mut radians = [0.0f64; N];
        radians.copy_from_slice(&v);
        Ok(Self { radians })
    }
}

impl<const N: usize> JointAngles<N> {
    pub fn new(radians: [f64; N]) -> Self {
        Self { radians }
    }

    /// All zeros — the "home" configuration most arms calibrate to.
    pub fn zero() -> Self {
        Self { radians: [0.0; N] }
    }

    /// Compile-time joint count. Trait-object code uses
    /// `RobotController::joint_count()` instead; this is for code
    /// that has the concrete type.
    pub const fn count() -> usize {
        N
    }

    /// Project to a heap-allocated Vec — used at the
    /// `RobotController` trait boundary where const-generic types
    /// don't fit.
    pub fn to_vec(&self) -> Vec<f64> {
        self.radians.to_vec()
    }
}

impl<const N: usize> Default for JointAngles<N> {
    fn default() -> Self {
        Self::zero()
    }
}

/// Per-joint min / max bounds. Defaults are NOT provided — every
/// robot defines its own limits from the vendor data sheet, and
/// running with `±∞` bounds would defeat the purpose.
///
/// Serde representation: `{"min": [..N..], "max": [..N..]}` — same
/// hand-rolled-array story as `JointAngles<N>` above.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointLimits<const N: usize> {
    pub min: [f64; N],
    pub max: [f64; N],
}

#[derive(Serialize, Deserialize)]
struct JointLimitsWire {
    min: Vec<f64>,
    max: Vec<f64>,
}

impl<const N: usize> Serialize for JointLimits<N> {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let wire = JointLimitsWire {
            min: self.min.to_vec(),
            max: self.max.to_vec(),
        };
        wire.serialize(ser)
    }
}

impl<'de, const N: usize> Deserialize<'de> for JointLimits<N> {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let wire = JointLimitsWire::deserialize(de)?;
        if wire.min.len() != N || wire.max.len() != N {
            return Err(serde::de::Error::invalid_length(
                wire.min.len().max(wire.max.len()),
                &format!("min/max arrays of length {N}").as_str(),
            ));
        }
        let mut min = [0.0f64; N];
        let mut max = [0.0f64; N];
        min.copy_from_slice(&wire.min);
        max.copy_from_slice(&wire.max);
        Ok(Self { min, max })
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum JointLimitError {
    /// `value` for joint `index` is outside `[min, max]`. UI shows
    /// "joint 3 angle 2.5 rad outside limits [-1.5, 1.5]".
    #[error("joint {index} value {value} out of range [{min}, {max}]")]
    OutOfRange {
        index: usize,
        value: f64,
        min: f64,
        max: f64,
    },
    /// The limits themselves are degenerate (`min > max`).
    /// Surfaces a config error rather than letting it silently
    /// reject every angle.
    #[error("joint {index} has degenerate limits: min={min} > max={max}")]
    DegenerateLimit { index: usize, min: f64, max: f64 },
    /// Non-finite (NaN or ±∞) value or limit. NaN comparisons
    /// always fail, so a NaN angle would slip through a naive
    /// `value < min || value > max` check; this catch is
    /// belt-and-braces.
    #[error("joint {index} has non-finite value or limit")]
    NonFinite { index: usize },
}

impl<const N: usize> JointLimits<N> {
    pub fn new(min: [f64; N], max: [f64; N]) -> Self {
        Self { min, max }
    }

    /// Convenience: symmetric ±`amplitude` on every joint. Useful
    /// for tests and for arms that genuinely have symmetric ranges
    /// (Universal Robots' UR5 is ±2π on every joint).
    pub fn symmetric(amplitude: f64) -> Self {
        Self {
            min: [-amplitude; N],
            max: [amplitude; N],
        }
    }

    /// Check every joint against its limit. Returns the FIRST
    /// violation; subsequent joints are not reported (intentional —
    /// see module docs).
    pub fn check(&self, angles: &JointAngles<N>) -> Result<(), JointLimitError> {
        for i in 0..N {
            // Non-finite catch first: a NaN angle slips through
            // `value < min || value > max` (NaN comparisons always
            // false).
            if !angles.radians[i].is_finite()
                || !self.min[i].is_finite()
                || !self.max[i].is_finite()
            {
                return Err(JointLimitError::NonFinite { index: i });
            }
            // Then the structural check: degenerate config first
            // (avoids reporting a "value outside [a,a-1]" when
            // really the config is broken).
            if self.min[i] > self.max[i] {
                return Err(JointLimitError::DegenerateLimit {
                    index: i,
                    min: self.min[i],
                    max: self.max[i],
                });
            }
            // Finally the actual range check.
            let v = angles.radians[i];
            if v < self.min[i] || v > self.max[i] {
                return Err(JointLimitError::OutOfRange {
                    index: i,
                    value: v,
                    min: self.min[i],
                    max: self.max[i],
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn zero_constructor_yields_all_zeros() {
        let j: JointAngles<6> = JointAngles::zero();
        assert_eq!(j.radians, [0.0; 6]);
    }

    #[test]
    fn count_returns_const_generic_n() {
        // The compile-time count is what specific impls use to
        // size joint loops; pin it so a typo can't make it lie.
        assert_eq!(JointAngles::<6>::count(), 6);
        assert_eq!(JointAngles::<3>::count(), 3);
        assert_eq!(JointAngles::<1>::count(), 1);
    }

    #[test]
    fn to_vec_preserves_order_and_count() {
        // Projection at the trait boundary must NOT reorder joints
        // — a downstream impl that swaps "shoulder" and "elbow"
        // would crash the arm.
        let j = JointAngles::<4>::new([0.1, 0.2, 0.3, 0.4]);
        let v = j.to_vec();
        assert_eq!(v, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn symmetric_limits_accept_zero_position() {
        // A symmetric ±π limit must accept the zero / home pose,
        // which is where every commissioning workflow starts.
        let lim: JointLimits<6> = JointLimits::symmetric(PI);
        assert!(lim.check(&JointAngles::zero()).is_ok());
    }

    #[test]
    fn limits_accept_in_range_angles() {
        let lim = JointLimits::<3>::symmetric(2.0);
        let j = JointAngles::new([1.5, -1.5, 0.0]);
        assert!(lim.check(&j).is_ok());
    }

    #[test]
    fn limits_reject_first_out_of_range_joint() {
        // The "FIRST violation" semantic — joint 1 is out of range,
        // joint 2 is also bad, but we only see joint 1 reported.
        let lim = JointLimits::<3>::symmetric(1.0);
        let j = JointAngles::new([0.5, 2.5, 3.0]);
        let err = lim.check(&j).unwrap_err();
        assert_eq!(
            err,
            JointLimitError::OutOfRange {
                index: 1,
                value: 2.5,
                min: -1.0,
                max: 1.0,
            }
        );
    }

    #[test]
    fn limits_reject_below_min() {
        // Symmetric check for the negative side.
        let lim = JointLimits::<2>::symmetric(0.5);
        let j = JointAngles::new([0.0, -1.0]);
        let err = lim.check(&j).unwrap_err();
        assert!(matches!(err, JointLimitError::OutOfRange { index: 1, .. }));
    }

    #[test]
    fn boundary_values_at_min_and_max_are_accepted_inclusive() {
        // The `<` and `>` (not `<=` `>=`) in the range check make
        // the bounds INCLUSIVE. Most vendor data sheets quote
        // inclusive ranges; matching them avoids "the manual says
        // ±2.5 but the safety relay refuses 2.4999999..." surprises.
        let lim = JointLimits::<2>::new([-1.0, -1.0], [1.0, 1.0]);
        let j = JointAngles::new([1.0, -1.0]);
        assert!(lim.check(&j).is_ok());
    }

    #[test]
    fn degenerate_limits_surface_as_typed_error() {
        // A misconfigured min > max would otherwise produce an
        // "OutOfRange" report for every angle, hiding the real bug.
        let lim = JointLimits::<2>::new([1.0, 0.0], [0.5, 1.0]);
        let j = JointAngles::zero();
        let err = lim.check(&j).unwrap_err();
        assert_eq!(
            err,
            JointLimitError::DegenerateLimit {
                index: 0,
                min: 1.0,
                max: 0.5,
            }
        );
    }

    #[test]
    fn nan_value_surfaces_non_finite_error() {
        // NaN bypasses naive comparison — explicit guard.
        let lim = JointLimits::<2>::symmetric(1.0);
        let j = JointAngles::new([f64::NAN, 0.0]);
        let err = lim.check(&j).unwrap_err();
        assert_eq!(err, JointLimitError::NonFinite { index: 0 });
    }

    #[test]
    fn infinite_value_surfaces_non_finite_error() {
        let lim = JointLimits::<2>::symmetric(1.0);
        let j = JointAngles::new([f64::INFINITY, 0.0]);
        let err = lim.check(&j).unwrap_err();
        assert_eq!(err, JointLimitError::NonFinite { index: 0 });
    }

    #[test]
    fn infinite_limit_surfaces_non_finite_error() {
        // A limit of ±∞ would defeat the safety check; catch it
        // explicitly rather than silently accept everything.
        let lim = JointLimits::<2>::new([f64::NEG_INFINITY, 0.0], [1.0, 1.0]);
        let j = JointAngles::new([0.0, 0.0]);
        let err = lim.check(&j).unwrap_err();
        assert_eq!(err, JointLimitError::NonFinite { index: 0 });
    }

    #[test]
    fn serde_round_trip_preserves_joint_angles() {
        let j = JointAngles::<6>::new([0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
        let s = serde_json::to_string(&j).unwrap();
        let back: JointAngles<6> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, j);
    }

    #[test]
    fn serde_round_trip_preserves_joint_limits() {
        let lim = JointLimits::<3>::new([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
        let s = serde_json::to_string(&lim).unwrap();
        let back: JointLimits<3> = serde_json::from_str(&s).unwrap();
        assert_eq!(back, lim);
    }

    #[test]
    fn type_safety_prevents_n_mismatch_at_compile_time() {
        // Documenting the load-bearing property via a const fn —
        // this would fail to compile if the const generics weren't
        // sound:
        //
        //   let a: JointAngles<6> = JointAngles::zero();
        //   let lim: JointLimits<3> = JointLimits::symmetric(1.0);
        //   lim.check(&a);  // ← compile error: expected <3>, got <6>
        //
        // The fact that this file builds is proof; we just assert
        // the count so the test surface acknowledges the property.
        const ARM_N: usize = 6;
        const AGV_N: usize = 3;
        assert_eq!(JointAngles::<ARM_N>::count(), 6);
        assert_eq!(JointAngles::<AGV_N>::count(), 3);
    }
}
