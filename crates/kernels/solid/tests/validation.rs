//! Input-failure gate for the convex-solid kernel: bad half-spaces must produce a typed error, not
//! silent undefined geometry. The valid cases (box, exact glacis slope) prove the happy path still
//! meshes to a clean closed solid.

use glam::Vec3;
use solid::{ConvexSolid, ConvexSolidError, Plane};
use vehicle_geometry::{MaterialRole, OPEN_OR_CLOSED_MESH, SmoothingGroup};

fn mesh(solid: &ConvexSolid) -> Result<vehicle_geometry::GeometryMesh, ConvexSolidError> {
    solid.to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
}

#[test]
fn a_valid_box_meshes_to_a_clean_solid() {
    let solid = ConvexSolid::box_at(Vec3::ZERO, Vec3::splat(1.0));
    let mesh = mesh(&solid).expect("a box is a valid bounded solid");
    mesh.validate_quality(OPEN_OR_CLOSED_MESH).expect("clean armour box");
    assert_eq!(mesh.triangle_count(), 12);
}

#[test]
fn an_exact_glacis_slope_meshes() {
    let slope = 60.0_f32.to_radians();
    let cut = Plane::new(Vec3::new(0.0, slope.cos(), slope.sin()), 1.0);
    let solid = ConvexSolid::box_at(Vec3::new(0.0, 0.6, 0.0), Vec3::splat(1.0)).clipped_by(cut);
    let mesh = mesh(&solid).expect("a clipped box stays valid");
    assert!(mesh.vertices().iter().any(|v| (v.normal - cut.normal).length() < 1.0e-3));
}

#[test]
fn a_clipped_box_remains_a_valid_closed_solid() {
    let cut = Plane::new(Vec3::new(1.0, 1.0, 0.0), 0.6);
    let solid = ConvexSolid::box_at(Vec3::ZERO, Vec3::splat(1.0)).clipped_by(cut);
    mesh(&solid).expect("clipped box meshes").validate_quality(OPEN_OR_CLOSED_MESH).expect("clean");
}

#[test]
fn a_duplicate_plane_is_collapsed_not_double_faced() {
    // Repeating one of the box's planes must not create an overlapping face; the solid still reads
    // as the same eight-corner box.
    let mut planes = ConvexSolid::box_at(Vec3::ZERO, Vec3::splat(1.0)).into_planes();
    planes.push(planes[0]);
    let solid = ConvexSolid::new(planes);
    let mesh = mesh(&solid).expect("a duplicate plane is harmless");
    assert_eq!(mesh.triangle_count(), 12, "the duplicate must not add a second top face");
}

#[test]
fn a_zero_normal_plane_is_rejected() {
    let solid = ConvexSolid::new(vec![
        Plane::new(Vec3::X, 1.0),
        Plane::new(-Vec3::X, 1.0),
        Plane::new(Vec3::Y, 1.0),
        Plane { normal: Vec3::ZERO, offset: 1.0 },
    ]);
    assert!(matches!(mesh(&solid), Err(ConvexSolidError::DegenerateNormal { .. })));
}

#[test]
fn plane_try_new_rejects_a_zero_normal() {
    assert!(matches!(
        Plane::try_new(Vec3::ZERO, 1.0),
        Err(ConvexSolidError::DegenerateNormal { index: 0 })
    ));
    assert!(matches!(
        Plane::try_new(Vec3::new(f32::NAN, 0.0, 0.0), 1.0),
        Err(ConvexSolidError::NonFinitePlane { .. })
    ));
}

#[test]
fn fewer_than_four_planes_is_rejected() {
    let solid = ConvexSolid::new(vec![Plane::new(Vec3::X, 1.0), Plane::new(Vec3::Y, 1.0)]);
    assert_eq!(mesh(&solid), Err(ConvexSolidError::TooFewPlanes(2)));
}

#[test]
fn contradictory_half_spaces_are_an_empty_intersection() {
    // x <= -1 and x >= 1 cannot both hold: the box is empty.
    let solid = ConvexSolid::new(vec![
        Plane::new(Vec3::X, -1.0),
        Plane::new(-Vec3::X, -1.0),
        Plane::new(Vec3::Y, 1.0),
        Plane::new(-Vec3::Y, 1.0),
        Plane::new(Vec3::Z, 1.0),
        Plane::new(-Vec3::Z, 1.0),
    ]);
    assert_eq!(mesh(&solid), Err(ConvexSolidError::EmptyIntersection));
}

#[test]
fn an_unbounded_three_plane_wedge_is_rejected() {
    // Three planes meeting at an edge run to infinity along that edge. (Caught as too few planes —
    // a wedge can never enclose a volume.)
    let wedge = ConvexSolid::new(vec![
        Plane::new(Vec3::Y, 0.0),
        Plane::new(Vec3::X, 0.0),
        Plane::new(Vec3::new(-1.0, -1.0, 0.0), 0.0),
    ]);
    assert!(mesh(&wedge).is_err());
}

#[test]
fn an_open_four_plane_prism_is_unbounded() {
    // Four side planes around Z but no caps: the prism runs to infinity along ±Z.
    let prism = ConvexSolid::new(vec![
        Plane::new(Vec3::X, 1.0),
        Plane::new(-Vec3::X, 1.0),
        Plane::new(Vec3::Y, 1.0),
        Plane::new(-Vec3::Y, 1.0),
    ]);
    assert_eq!(mesh(&prism), Err(ConvexSolidError::Unbounded));
}

/// A zero chamfer means "no chamfer", not "fail". The chamfer planes would otherwise pass
/// exactly through the box edges, leaving two-corner face rings that `to_mesh` rejects —
/// so a caller sweeping the parameter to zero used to get `DegenerateFace` instead of a box.
#[test]
fn a_zero_chamfer_degrades_to_a_plain_box_instead_of_failing() {
    let center = Vec3::new(0.2, 1.1, -0.4);
    let half = Vec3::new(0.30, 0.22, 0.14);
    let plain = mesh(&ConvexSolid::box_at(center, half));
    for chamfer in [0.0, 1.0e-6, 1.0e-4] {
        let built = mesh(&solid::chamfered_box(center, half, chamfer))
            .expect("a chamfer of zero is a plain box, not an error");
        let plain = plain.as_ref().expect("the reference box builds");
        assert_eq!(
            built.triangle_count(),
            plain.triangle_count(),
            "chamfer {chamfer} must produce exactly the plain box"
        );
    }
    // A real chamfer still cuts corners — the degradation is only for sub-epsilon values.
    let chamfered = mesh(&solid::chamfered_box(center, half, 0.03)).expect("a real chamfer builds");
    assert!(
        chamfered.triangle_count() > plain.as_ref().unwrap().triangle_count(),
        "a genuine chamfer adds faces"
    );
}
