//! Ergonomic constructors and combinators for building an [`Sdf`] tree by chaining.
//!
//! Keeping the builder here leaves `node.rs` focused on the representation and its evaluation.

use glam::{Quat, Vec3};

use crate::Sdf;

/// Why an SDF primitive or operation cannot be constructed from the given parameters.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum SdfError {
    #[error("radius must be finite and non-negative")]
    NegativeRadius,
    #[error("half-extent must be finite and non-negative")]
    NegativeHalfExtent,
    #[error("half-space normal must be finite and non-zero")]
    ZeroNormal,
    #[error("rigid transform translation must be finite")]
    NonFiniteTransform,
    #[error("rotation quaternion must be finite and normalizable")]
    NonNormalizableRotation,
    #[error("smooth-blend radius must be finite")]
    NonFiniteRadius,
}

impl Sdf {
    pub fn sphere(radius: f32) -> Self {
        Sdf::Sphere { radius }
    }

    /// Validated sphere: the radius must be finite and non-negative.
    pub fn try_sphere(radius: f32) -> Result<Self, SdfError> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(SdfError::NegativeRadius);
        }
        Ok(Sdf::Sphere { radius })
    }

    /// Validated box: every half-extent must be finite and non-negative.
    pub fn try_cuboid(half: Vec3) -> Result<Self, SdfError> {
        if !half.is_finite() || half.min_element() < 0.0 {
            return Err(SdfError::NegativeHalfExtent);
        }
        Ok(Sdf::Cuboid { half })
    }

    /// Validated half-space: the normal must be finite and non-zero (it is normalised).
    pub fn try_half_space(normal: Vec3, offset: f32) -> Result<Self, SdfError> {
        if !normal.is_finite() || !offset.is_finite() || normal.length() < 1.0e-6 {
            return Err(SdfError::ZeroNormal);
        }
        Ok(Sdf::HalfSpace { normal: normal.normalize(), offset })
    }

    /// Validated cylinder: radius and half-height must be finite and non-negative.
    pub fn try_cylinder(radius: f32, half_height: f32) -> Result<Self, SdfError> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(SdfError::NegativeRadius);
        }
        if !half_height.is_finite() || half_height < 0.0 {
            return Err(SdfError::NegativeHalfExtent);
        }
        Ok(Sdf::Cylinder { radius, half_height })
    }

    /// Validated rigid transform: finite translation and a finite, normalizable rotation.
    pub fn try_transform(self, rotation: Quat, translation: Vec3) -> Result<Self, SdfError> {
        if !translation.is_finite() {
            return Err(SdfError::NonFiniteTransform);
        }
        let len = rotation.length();
        if !len.is_finite() || len < 1.0e-6 {
            return Err(SdfError::NonNormalizableRotation);
        }
        Ok(Sdf::Rigid { rotation: rotation.normalize(), translation, node: Box::new(self) })
    }

    /// Validated smooth union: the blend radius must be finite.
    pub fn try_smooth_union(self, other: Sdf, radius: f32) -> Result<Self, SdfError> {
        if !radius.is_finite() {
            return Err(SdfError::NonFiniteRadius);
        }
        Ok(Sdf::SmoothUnion { a: Box::new(self), b: Box::new(other), radius })
    }

    /// Validated smooth subtract: the blend radius must be finite.
    pub fn try_smooth_subtract(self, other: Sdf, radius: f32) -> Result<Self, SdfError> {
        if !radius.is_finite() {
            return Err(SdfError::NonFiniteRadius);
        }
        Ok(Sdf::SmoothSubtract { a: Box::new(self), b: Box::new(other), radius })
    }

    pub fn cuboid(half: Vec3) -> Self {
        Sdf::Cuboid { half }
    }

    /// A flat plate facing `normal` (need not be unit — it is normalised here), at signed distance
    /// `offset` from the origin along that normal. The normal *is* the armour-plate slope.
    pub fn half_space(normal: Vec3, offset: f32) -> Self {
        Sdf::HalfSpace { normal: normal.normalize(), offset }
    }

    pub fn cylinder(radius: f32, half_height: f32) -> Self {
        Sdf::Cylinder { radius, half_height }
    }

    pub fn translate(self, translation: Vec3) -> Self {
        self.transform(Quat::IDENTITY, translation)
    }

    pub fn rotate(self, rotation: Quat) -> Self {
        self.transform(rotation, Vec3::ZERO)
    }

    pub fn transform(self, rotation: Quat, translation: Vec3) -> Self {
        Sdf::Rigid { rotation, translation, node: Box::new(self) }
    }

    pub fn union(self, other: Sdf) -> Self {
        Sdf::Union(Box::new(self), Box::new(other))
    }

    pub fn intersect(self, other: Sdf) -> Self {
        Sdf::Intersect(Box::new(self), Box::new(other))
    }

    pub fn subtract(self, other: Sdf) -> Self {
        Sdf::Subtract(Box::new(self), Box::new(other))
    }

    pub fn smooth_union(self, other: Sdf, radius: f32) -> Self {
        Sdf::SmoothUnion { a: Box::new(self), b: Box::new(other), radius }
    }

    pub fn smooth_subtract(self, other: Sdf, radius: f32) -> Self {
        Sdf::SmoothSubtract { a: Box::new(self), b: Box::new(other), radius }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1.0e-5;

    #[test]
    fn translate_places_the_shape() {
        let s = Sdf::sphere(1.0).translate(Vec3::new(5.0, 0.0, 0.0));
        assert!(
            (s.eval(Vec3::new(5.0, 0.0, 0.0)) + 1.0).abs() < EPS,
            "centre moved to translation"
        );
        assert!(s.eval(Vec3::ZERO) > 0.0, "origin is now outside");
    }

    #[test]
    fn rotate_preserves_distance_metric() {
        // A rigid rotation must not change the distance to a sphere centred at the origin.
        let s = Sdf::sphere(1.0).rotate(Quat::from_rotation_z(0.7));
        assert!((s.eval(Vec3::new(3.0, 0.0, 0.0)) - 2.0).abs() < EPS);
    }

    #[test]
    fn subtract_carves_a_hole() {
        // A box with a sphere bitten out of its centre: the centre is now outside (positive).
        let body = Sdf::cuboid(Vec3::splat(1.0));
        let bite = Sdf::sphere(0.5);
        let carved = body.subtract(bite);
        assert!(carved.eval(Vec3::ZERO) > 0.0, "carved centre is hollow");
        assert!(carved.eval(Vec3::new(0.9, 0.0, 0.0)) < 0.0, "the shell wall is still solid");
    }

    #[test]
    fn intersect_keeps_only_the_overlap() {
        let a = Sdf::sphere(1.0).translate(Vec3::new(-0.5, 0.0, 0.0));
        let b = Sdf::sphere(1.0).translate(Vec3::new(0.5, 0.0, 0.0));
        let lens = a.intersect(b);
        assert!(lens.eval(Vec3::ZERO) < 0.0, "overlap centre is inside");
        assert!(lens.eval(Vec3::new(-1.2, 0.0, 0.0)) > 0.0, "outside one sphere is excluded");
    }

    #[test]
    fn a_plate_intersection_builds_a_sloped_wedge() {
        // A sloped front plate plus a ground plane make a crisp wedge — the kind of construction a
        // glacis/hull front uses. `front` keeps the half below its raked face (dot < 0); `floor`
        // (normal -Y) keeps the half above y = 0. Their intersection is solid where both hold.
        let front = Sdf::half_space(Vec3::new(0.0, 1.0, 1.0), 0.0);
        let floor = Sdf::half_space(Vec3::new(0.0, -1.0, 0.0), 0.0);
        let wedge = front.intersect(floor);
        assert!(
            wedge.eval(Vec3::new(0.0, 0.5, -2.0)) < 0.0,
            "above ground, behind the slope: solid"
        );
        assert!(wedge.eval(Vec3::new(0.0, 1.0, 1.0)) > 0.0, "out past the glacis is empty");
    }
}
