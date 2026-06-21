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
    })
}

#[test]
fn a_straight_capped_pipe_is_a_closed_manifold() {
    let path = SweepPath { points: vec![Vec3::ZERO, Vec3::Z, Vec3::Z * 2.0], closed: false };
    let mesh = sweep(&path, SweepFrameMode::ParallelTransport, SweepCaps::Both).expect("pipe");
    mesh.validate_quality(CLOSED_SMOOTH_MESH).expect("a capped pipe is a closed manifold");
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
    })
    .unwrap_err();
    assert_eq!(err, SweepError::NonConvexSection);
}
