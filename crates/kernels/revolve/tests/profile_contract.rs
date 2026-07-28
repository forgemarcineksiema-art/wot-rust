//! The validated revolution contract: a clean profile revolves about any cardinal axis into a
//! closed, unit-normalled surface, and every malformed input is a typed error rather than garbage.

use glam::Vec3;
use revolve::{ProfilePoint, RevolveCaps, RevolveError, RevolveProfile, try_revolve};
use vehicle_geometry::{CLOSED_SMOOTH_MESH, MaterialRole, OPEN_OR_CLOSED_MESH, SmoothingGroup};

fn cylinder() -> RevolveProfile {
    RevolveProfile::new(
        vec![ProfilePoint::new(-0.5, 0.4), ProfilePoint::new(0.5, 0.4)],
        RevolveCaps::Capped,
    )
}

fn build(axis: Vec3) -> Result<vehicle_geometry::GeometryMesh, RevolveError> {
    try_revolve(axis, &cylinder(), 16, MaterialRole::Rubber, SmoothingGroup(5))
}

#[test]
fn a_capped_cylinder_revolves_about_each_cardinal_axis_into_a_closed_manifold() {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        let mesh = build(axis).expect("clean cylinder revolves");
        mesh.validate_quality(CLOSED_SMOOTH_MESH)
            .unwrap_or_else(|e| panic!("axis {axis:?} not a closed manifold: {e}"));
    }
}

#[test]
fn capped_end_discs_face_outward_along_the_axis() {
    let mesh = try_revolve(Vec3::X, &cylinder(), 16, MaterialRole::Rubber, SmoothingGroup(5))
        .expect("cylinder");
    // The cap-centre apexes sit alone on the axis; their welded normal is the pure outward cap.
    let apex = |x: f32| {
        *mesh
            .vertices()
            .iter()
            .find(|v| {
                (v.position.x - x).abs() < 1.0e-4
                    && v.position.y.abs() < 1.0e-4
                    && v.position.z.abs() < 1.0e-4
            })
            .expect("cap-centre apex on the axis")
    };
    assert!(apex(0.5).normal.x > 0.9, "+X cap faces outward");
    assert!(apex(-0.5).normal.x < -0.9, "-X cap faces outward");
}

#[test]
fn an_open_profile_leaves_the_ends_open() {
    let open = RevolveProfile::new(
        vec![ProfilePoint::new(-0.5, 0.4), ProfilePoint::new(0.5, 0.4)],
        RevolveCaps::Open,
    );
    let mesh = try_revolve(Vec3::Z, &open, 16, MaterialRole::Rubber, SmoothingGroup(5))
        .expect("open tube");
    let report = mesh.quality_report(OPEN_OR_CLOSED_MESH);
    assert_eq!(report.boundary_edges, 2 * 16, "both ends of the open tube are boundary rings");
}

#[test]
fn arc_length_is_cumulative_along_the_profile() {
    let profile = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.0), ProfilePoint::new(0.0, 1.0), ProfilePoint::new(1.0, 1.0)],
        RevolveCaps::Open,
    );
    let lengths = profile.arc_lengths();
    assert_eq!(lengths, vec![0.0, 1.0, 2.0]);
}

#[test]
fn a_zero_axis_is_rejected() {
    assert_eq!(build(Vec3::ZERO), Err(RevolveError::InvalidAxis));
    assert_eq!(build(Vec3::new(f32::NAN, 0.0, 0.0)), Err(RevolveError::InvalidAxis));
}

#[test]
fn too_few_segments_is_rejected() {
    let r = try_revolve(Vec3::Z, &cylinder(), 2, MaterialRole::Rubber, SmoothingGroup(5));
    assert_eq!(r, Err(RevolveError::TooFewSegments));
}

#[test]
fn too_few_points_is_rejected() {
    let p = RevolveProfile::new(vec![ProfilePoint::new(0.0, 0.4)], RevolveCaps::Open);
    assert_eq!(
        try_revolve(Vec3::Z, &p, 16, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::TooFewPoints)
    );
}

#[test]
fn a_negative_radius_is_rejected() {
    let p = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.4), ProfilePoint::new(1.0, -0.1)],
        RevolveCaps::Open,
    );
    assert_eq!(
        try_revolve(Vec3::Z, &p, 16, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::NegativeRadius)
    );
}

#[test]
fn a_non_finite_point_is_rejected() {
    let p = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.4), ProfilePoint::new(f32::INFINITY, 0.4)],
        RevolveCaps::Open,
    );
    assert_eq!(
        try_revolve(Vec3::Z, &p, 16, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::InvalidPoint { index: 1 })
    );
}

#[test]
fn duplicated_adjacent_points_are_rejected() {
    let p = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.4), ProfilePoint::new(0.0, 0.4)],
        RevolveCaps::Open,
    );
    assert_eq!(
        try_revolve(Vec3::Z, &p, 16, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::CoincidentPoints)
    );
}

#[test]
fn a_radius_zero_apex_is_preserved_as_a_pointed_cap() {
    // A cone: a radius-zero tip is a legitimate non-degenerate transition into the next ring.
    let cone = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.0), ProfilePoint::new(1.0, 0.5)],
        RevolveCaps::Capped,
    );
    let mesh = try_revolve(Vec3::Y, &cone, 16, MaterialRole::Rubber, SmoothingGroup(5))
        .expect("a cone revolves");
    assert!(mesh.triangle_count() > 0);
    assert_eq!(mesh.quality_report(OPEN_OR_CLOSED_MESH).degenerate_triangles, 0);
}

/// The silent-garbage hole this validation closes: a profile with a positive but sub-weld-grid
/// radius used to pass validation and mesh into degenerate triangles and non-manifold edges —
/// a "valid" mesh that no contract could catch, because the weld had already fused the ring
/// into a single point while the fan still treated it as a ring.
#[test]
fn a_sub_millimetre_ring_is_rejected_instead_of_meshing_into_garbage() {
    let hair = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 1.0e-6), ProfilePoint::new(0.5, 0.4)],
        RevolveCaps::Capped,
    );
    assert!(matches!(
        try_revolve(Vec3::Y, &hair, 16, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::DegenerateRadius { index: 0, .. })
    ));
}

/// The same defect reached from the other direction: a legal radius cut into so many segments
/// that adjacent ring vertices land inside one weld cell. Open caps here, so the subject is
/// purely the RING — a flat end disc of that radius has its own (separate, older) limit: the
/// quality contract's minimum triangle area.
#[test]
fn a_legal_radius_sliced_too_finely_is_rejected_before_it_collapses() {
    let fine = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.0015), ProfilePoint::new(0.2, 0.0015)],
        RevolveCaps::Open,
    );
    // 0.0015 m radius over 64 segments: chord ~0.15 mm, under the 0.24 mm weld grid.
    assert!(matches!(
        try_revolve(Vec3::Y, &fine, 64, MaterialRole::Rubber, SmoothingGroup(5)),
        Err(RevolveError::RingCollapsesUnderWeld { .. })
    ));
    // The same part at a sane tessellation is fine — the gate rejects collapse, not small parts.
    let mesh = try_revolve(Vec3::Y, &fine, 8, MaterialRole::Rubber, SmoothingGroup(5))
        .expect("a thin wire-scale tube still revolves");
    assert_eq!(mesh.quality_report(OPEN_OR_CLOSED_MESH).degenerate_triangles, 0);
}

/// Small real parts (bolt heads, pin ends, wire) must keep working — the threshold is the weld
/// grid, not a ban on detail.
#[test]
fn millimetre_scale_detail_still_revolves_cleanly() {
    let bolt = RevolveProfile::new(
        vec![ProfilePoint::new(0.0, 0.006), ProfilePoint::new(0.004, 0.006)],
        RevolveCaps::Capped,
    );
    let mesh = try_revolve(Vec3::Y, &bolt, 12, MaterialRole::RolledArmor, SmoothingGroup(1))
        .expect("a 6 mm bolt head revolves");
    let report = mesh.quality_report(CLOSED_SMOOTH_MESH);
    assert_eq!(report.degenerate_triangles, 0);
    assert_eq!(report.non_manifold_edges, 0);
}
