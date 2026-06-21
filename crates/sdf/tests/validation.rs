//! Construction validation and analytic-gradient contract for the SDF kernel.

use glam::{Quat, Vec3};
use sdf::{Sdf, SdfError, SignedDistance};

#[test]
fn negative_and_non_finite_radii_are_rejected() {
    assert_eq!(Sdf::try_sphere(-1.0), Err(SdfError::NegativeRadius));
    assert_eq!(Sdf::try_sphere(f32::NAN), Err(SdfError::NegativeRadius));
    assert!(Sdf::try_sphere(2.0).is_ok());
}

#[test]
fn negative_half_extents_are_rejected() {
    assert_eq!(Sdf::try_cuboid(Vec3::new(1.0, -1.0, 1.0)), Err(SdfError::NegativeHalfExtent));
    assert!(Sdf::try_cuboid(Vec3::splat(0.5)).is_ok());
}

#[test]
fn a_zero_half_space_normal_is_rejected() {
    assert_eq!(Sdf::try_half_space(Vec3::ZERO, 0.0), Err(SdfError::ZeroNormal));
    assert!(Sdf::try_half_space(Vec3::Y, 1.0).is_ok());
}

#[test]
fn a_non_finite_transform_or_rotation_is_rejected() {
    let s = Sdf::sphere(1.0);
    assert_eq!(
        s.clone().try_transform(Quat::IDENTITY, Vec3::new(f32::NAN, 0.0, 0.0)),
        Err(SdfError::NonFiniteTransform)
    );
    assert_eq!(
        s.clone().try_transform(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0), Vec3::ZERO),
        Err(SdfError::NonNormalizableRotation)
    );
    assert!(s.try_transform(Quat::from_rotation_y(0.5), Vec3::X).is_ok());
}

#[test]
fn a_non_finite_smooth_radius_is_rejected() {
    let a = Sdf::sphere(1.0);
    let b = Sdf::sphere(1.0);
    assert_eq!(
        a.clone().try_smooth_union(b.clone(), f32::INFINITY),
        Err(SdfError::NonFiniteRadius)
    );
    assert!(a.try_smooth_union(b, 0.3).is_ok());
}

#[test]
fn sphere_gradient_points_radially_outward() {
    let s = Sdf::sphere(2.0);
    let g = s.gradient(Vec3::new(3.0, 0.0, 0.0));
    assert!((g - Vec3::X).length() < 1.0e-5);
}

#[test]
fn half_space_gradient_is_the_plate_normal() {
    let n = Vec3::new(0.0, 1.0, 1.0).normalize();
    let plate = Sdf::half_space(n, 0.0);
    assert!((plate.gradient(Vec3::new(0.0, 2.0, 2.0)) - n).length() < 1.0e-5);
}

#[test]
fn cuboid_gradient_faces_out_through_the_nearest_face() {
    let b = Sdf::cuboid(Vec3::splat(1.0));
    // Outside past the +X face.
    assert!((b.gradient(Vec3::new(2.0, 0.0, 0.0)) - Vec3::X).length() < 1.0e-5);
    // Just inside near the +X face: still out through +X.
    assert!((b.gradient(Vec3::new(0.9, 0.0, 0.0)) - Vec3::X).length() < 1.0e-5);
}

#[test]
fn analytic_gradients_match_finite_differences_on_a_blend() {
    // For a smooth union the analytic blended gradient should track a numeric gradient closely
    // away from the exact seam.
    let blend = Sdf::sphere(1.0)
        .translate(Vec3::new(-0.6, 0.0, 0.0))
        .smooth_union(Sdf::sphere(1.0).translate(Vec3::new(0.6, 0.0, 0.0)), 0.4);
    let p = Vec3::new(0.0, 0.8, 0.0);
    let analytic = blend.gradient(p);
    let h = 1.0e-3;
    let numeric = Vec3::new(
        blend.eval(p + Vec3::X * h) - blend.eval(p - Vec3::X * h),
        blend.eval(p + Vec3::Y * h) - blend.eval(p - Vec3::Y * h),
        blend.eval(p + Vec3::Z * h) - blend.eval(p - Vec3::Z * h),
    )
    .normalize();
    assert!(analytic.dot(numeric) > 0.99, "analytic {analytic:?} vs numeric {numeric:?}");
}

#[test]
fn a_rotated_shape_rotates_its_gradient() {
    let rot = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    // Rotate a half-space whose normal is +X; the gradient should rotate to ~+Y.
    let plate = Sdf::half_space(Vec3::X, 0.0).rotate(rot);
    let g = plate.gradient(Vec3::new(0.0, 1.0, 0.0));
    assert!((g - Vec3::Y).length() < 1.0e-4, "rotated gradient {g:?}");
}

#[test]
fn the_trait_object_exposes_both_eval_and_gradient() {
    let s: &dyn SignedDistance = &Sdf::sphere(1.0);
    assert!((s.eval(Vec3::new(2.0, 0.0, 0.0)) - 1.0).abs() < 1.0e-5);
    assert!((s.gradient(Vec3::new(2.0, 0.0, 0.0)) - Vec3::X).length() < 1.0e-5);
}
