//! The path-sweep contract: clean inputs sweep into a valid band; malformed inputs are typed errors.

use glam::{Vec2, Vec3};
use sweep::{SweepCaps, SweepError, SweepFrameMode, SweepPath, SweepSection, SweepSpec, try_sweep};
use vehicle_geometry::{CLOSED_SMOOTH_MESH, MaterialRole, OPEN_OR_CLOSED_MESH, SmoothingGroup};

fn square_section() -> SweepSection {
    SweepSection {
        points: vec![
            Vec2::new(-0.1, -0.2),
            Vec2::new(0.1, -0.2),
            Vec2::new(0.1, 0.2),
            Vec2::new(-0.1, 0.2),
        ],
        closed: true,
    }
}

fn sweep(
    path: &SweepPath,
    mode: SweepFrameMode,
    caps: SweepCaps,
) -> Result<vehicle_geometry::GeometryMesh, SweepError> {
    try_sweep(&SweepSpec {
        path,
        section: &square_section(),
        frame_mode: mode,
        caps,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        section_scale: None,
    })
}

#[test]
fn a_straight_capped_pipe_is_a_closed_manifold() {
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    let mesh = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both).expect("pipe");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a capped pipe is a closed manifold");
}

#[test]
fn a_clockwise_wound_section_still_sweeps_faces_outward() {
    // `square_section` is CCW. A caller that lists the SAME convex outline the other way round
    // (clockwise, negative signed area) must get the identical outward-facing band — the kernel
    // normalizes winding, as `loft` and `extrude` do. Before that fix the CW section passed the
    // winding-agnostic convexity check and then built inward-facing, lit-from-behind walls, which
    // `CLOSED_SMOOTH_MESH`'s outward-winding gate rejects.
    let cw_section = SweepSection {
        points: vec![
            Vec2::new(-0.1, 0.2),
            Vec2::new(0.1, 0.2),
            Vec2::new(0.1, -0.2),
            Vec2::new(-0.1, -0.2),
        ],
        closed: true,
    };
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    let mesh = try_sweep(&SweepSpec {
        path: &path,
        section: &cw_section,
        frame_mode: SweepFrameMode::ParallelTransport,
        caps: SweepCaps::Both,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        section_scale: None,
    })
    .expect("a clockwise section is a valid convex outline");
    mesh.validate_quality(CLOSED_SMOOTH_MESH)
        .expect("a clockwise-wound section still faces outward once winding is normalized");
}

#[test]
fn an_open_pipe_without_caps_has_two_boundary_rings() {
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    let mesh = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Open).expect("pipe");
    let report = mesh.quality_report(OPEN_OR_CLOSED_MESH);
    assert_eq!(report.boundary_edges, 2 * 4, "both open ends are 4-edge section rings");
}

#[test]
fn a_planar_closed_rounded_rectangle_sweeps_into_a_closed_band() {
    // A rounded-rectangle loop in the Y-Z plane, swept with up = X (a track-belt-shaped path).
    let mut points = Vec::new();
    for i in 0..48 {
        let a = i as f32 / 48.0 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        points.push(Vec3::new(0.0, 0.5 * s, 1.5 * c));
    }
    let path = SweepPath { points, closed: true };
    let mesh = sweep(&path, SweepFrameMode::FixedUp(Vec3::X), SweepCaps::Open).expect("loop");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a closed swept band is a closed manifold");
}

#[test]
fn the_seam_of_a_closed_parallel_transport_loop_is_clean() {
    // A closed planar loop swept with parallel transport: the distributed twist must leave the
    // seam a clean closed manifold (no boundary or non-manifold edges at the wrap).
    let mut points = Vec::new();
    for i in 0..32 {
        let a = i as f32 / 32.0 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        points.push(Vec3::new(c, 0.0, s));
    }
    let path = SweepPath { points, closed: true };
    let mesh = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Open).expect("torus");
    let report = mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("clean closed torus");
    assert_eq!(report.boundary_edges, 0);
}

#[test]
fn the_sweep_is_deterministic() {
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    let a = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both).unwrap();
    let b = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both).unwrap();
    assert_eq!(a, b);
}

#[test]
fn a_zero_length_segment_is_rejected() {
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::ZERO, Vec3::Z], closed: false };
    assert_eq!(
        sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both),
        Err(SweepError::DegenerateSegment { index: 0 })
    );
}

#[test]
fn too_few_path_points_are_rejected() {
    let path = SweepPath { points: vec![Vec3::ZERO], closed: false };
    assert_eq!(
        sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both),
        Err(SweepError::TooFewPathPoints)
    );
}

#[test]
fn an_up_vector_parallel_to_the_path_is_rejected() {
    // Path runs along Z; up = Z is parallel, so the fixed-up projection collapses.
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    assert_eq!(
        sweep(&path, SweepFrameMode::FixedUp(Vec3::Z), SweepCaps::Open),
        Err(SweepError::InvalidUp)
    );
}

#[test]
fn a_concave_section_is_rejected() {
    let concave = SweepSection {
        points: vec![
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 0.2),
            Vec2::new(-1.0, 1.0),
        ],
        closed: true,
    };
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z], closed: false };
    let err = try_sweep(&SweepSpec {
        path: &path,
        section: &concave,
        frame_mode: SweepFrameMode::ParallelTransport,
        caps: SweepCaps::Open,
        material: MaterialRole::TrackMetal,
        smoothing: SmoothingGroup::hard_edges(),
        section_scale: None,
    })
    .unwrap_err();
    assert_eq!(err, SweepError::NonConvexSection);
}

/// A constant section can only make tubes. The parts a tank carries that are NOT tubes — a
/// canvas mantlet cover flaring from the barrel out to the turret face, a tapering line, a
/// bellows — are the same sweep with the section growing along the path.
#[test]
fn a_section_scale_flares_the_sweep_along_its_path() {
    let path = SweepPath {
        points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.3), Vec3::new(0.0, 0.0, 0.6)],
        closed: false,
    };
    let section = square_section();
    let mut spec = SweepSpec {
        path: &path,
        section: &section,
        frame_mode: SweepFrameMode::FixedUp(Vec3::Y),
        caps: SweepCaps::Both,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(3),
        section_scale: None,
    };

    let straight = try_sweep(&spec).expect("a constant tube sweeps");
    let straight_bounds = straight.bounds().expect("bounds");

    let flare = [1.0, 2.0, 3.0];
    spec.section_scale = Some(&flare);
    let flared = try_sweep(&spec).expect("a flaring cover sweeps");
    let flared_bounds = flared.bounds().expect("bounds");

    assert_eq!(
        flared.triangle_count(),
        straight.triangle_count(),
        "the taper changes the SHAPE, not the topology"
    );
    assert!(
        flared_bounds.max.x > straight_bounds.max.x * 2.5,
        "the far end must actually flare: {:.3} vs {:.3}",
        flared_bounds.max.x,
        straight_bounds.max.x
    );
    // Station-by-station: the scale table IS the profile. 1 -> 2 -> 3 must read as 1:2:3,
    // whichever way the transport frame happens to lay the section's axes out.
    let half_width_at = |z: f32| {
        flared
            .vertices()
            .iter()
            .filter(|vertex| (vertex.position.z - z).abs() < 0.01)
            .map(|vertex| vertex.position.x.abs())
            .fold(0.0_f32, f32::max)
    };
    let (near, mid, far) = (half_width_at(0.0), half_width_at(0.3), half_width_at(0.6));
    assert!(near > 0.0, "the near station must exist");
    assert!(
        (mid / near - 2.0).abs() < 0.01 && (far / near - 3.0).abs() < 0.01,
        "the scale table is the profile: got 1 : {:.2} : {:.2}",
        mid / near,
        far / near
    );
    assert_eq!(
        flared.quality_report(OPEN_OR_CLOSED_MESH).degenerate_triangles,
        0,
        "a flared sweep is still a clean surface"
    );
}

#[test]
fn a_malformed_section_scale_is_a_typed_error() {
    let path = SweepPath {
        points: vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.0, 0.0, 1.0)],
        closed: false,
    };
    let section = square_section();
    let spec = |scale: &'static [f32]| SweepSpec {
        path: &path,
        section: &section,
        frame_mode: SweepFrameMode::FixedUp(Vec3::Y),
        caps: SweepCaps::Both,
        material: MaterialRole::CastArmor,
        smoothing: SmoothingGroup(3),
        section_scale: Some(scale),
    };

    // One scale per station, or the tail of the sweep silently stays at full size.
    assert!(matches!(
        try_sweep(&spec(&[1.0, 2.0])),
        Err(SweepError::SectionScaleLengthMismatch { given: 2, expected: 3 })
    ));
    assert!(matches!(
        try_sweep(&spec(&[1.0, 0.0, 2.0])),
        Err(SweepError::InvalidSectionScale { index: 1, .. })
    ));
}
