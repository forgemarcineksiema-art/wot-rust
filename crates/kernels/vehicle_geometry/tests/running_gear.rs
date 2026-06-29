//! Locking tests for the animatable running gear: placement counts, wheel spin, belt wrap,
//! per-side independence, and determinism.

use game_core::VehicleKind;
use glam::{Mat4, Vec3};
use vehicle_geometry::{
    GearPart, MaterialRole, RunningGearKinematics, idler_unit_mesh, road_wheel_unit_mesh,
    running_gear_placements, sprocket_unit_mesh, track_link_unit_mesh,
};

fn t55() -> RunningGearKinematics {
    RunningGearKinematics::for_vehicle(VehicleKind::T55A).expect("T-55A has blueprint running gear")
}

fn count(placements: &[vehicle_geometry::GearPlacement], part: GearPart) -> usize {
    placements.iter().filter(|p| p.part == part).count()
}

fn mats_close(a: Mat4, b: Mat4) -> bool {
    a.to_cols_array().iter().zip(b.to_cols_array()).all(|(x, y)| (x - y).abs() < 1.0e-3)
}

#[test]
fn non_blueprint_vehicles_have_no_animated_gear() {
    // The legacy path keeps its static baked gear; only blueprint vehicles animate.
    assert!(RunningGearKinematics::for_vehicle(VehicleKind::TigerII).is_none());
    assert!(RunningGearKinematics::for_vehicle(VehicleKind::T55A).is_some());
}

#[test]
fn placements_cover_both_sides_for_every_part() {
    let kin = t55();
    let placements = running_gear_placements(&kin, 0.0, 0.0);

    // Two sides: road wheels = wheel_zs * 2, plus front idler and rear drive sprocket per side.
    assert_eq!(count(&placements, GearPart::RoadWheel), kin.wheel_zs.len() * 2);
    assert_eq!(count(&placements, GearPart::Idler), 2);
    assert_eq!(count(&placements, GearPart::Sprocket), 2);
    assert_eq!(count(&placements, GearPart::Link), kin.link_count() * 2);
}

#[test]
fn a_full_wheel_circumference_of_travel_returns_the_wheel_to_its_pose() {
    let kin = t55();
    let rest = running_gear_placements(&kin, 0.0, 0.0);
    let full_turn = 2.0 * std::f32::consts::PI * kin.wheel_radius;
    let rolled = running_gear_placements(&kin, full_turn, full_turn);

    // Road wheels spin by distance / radius, so one circumference of travel is a full revolution
    // back to the same orientation.
    for (a, b) in rest.iter().zip(&rolled).filter(|(a, _)| a.part == GearPart::RoadWheel) {
        assert!(mats_close(a.transform, b.transform), "a wheel should return after one revolution");
    }
}

#[test]
fn a_small_advance_moves_the_wheels() {
    let kin = t55();
    let rest = running_gear_placements(&kin, 0.0, 0.0);
    let nudged = running_gear_placements(&kin, 0.1, 0.1);

    let moved = rest
        .iter()
        .zip(&nudged)
        .filter(|(a, _)| a.part == GearPart::RoadWheel)
        .any(|(a, b)| !mats_close(a.transform, b.transform));
    assert!(moved, "a small distance must visibly spin the wheels");
}

#[test]
fn links_wrap_continuously_around_the_loop() {
    let kin = t55();
    let rest = running_gear_placements(&kin, 0.0, 0.0);
    let looped = running_gear_placements(&kin, kin.belt_length(), kin.belt_length());

    // Advancing the links by exactly one loop length lands them back on the same positions.
    for (a, b) in rest.iter().zip(&looped).filter(|(a, _)| a.part == GearPart::Link) {
        assert!(mats_close(a.transform, b.transform), "links must wrap continuously over the loop");
    }
}

#[test]
fn the_two_tracks_run_independently() {
    let kin = t55();
    // Pivot in place: left runs back, right runs forward.
    let placements = running_gear_placements(&kin, -1.0, 1.0);
    let wheels: Vec<_> = placements.iter().filter(|p| p.part == GearPart::RoadWheel).collect();

    // Right side wheels come first; a left/right phase mismatch must produce different spins.
    let right = wheels.first().expect("a right wheel").transform;
    let left = wheels.last().expect("a left wheel").transform;
    assert!(!mats_close(right, left), "opposite per-side phases must spin the tracks differently");
}

#[test]
fn placements_are_deterministic() {
    let kin = t55();
    assert_eq!(running_gear_placements(&kin, 3.3, -1.7), running_gear_placements(&kin, 3.3, -1.7));
}

#[test]
fn unit_meshes_are_finite_and_non_empty() {
    let kin = t55();
    for mesh in [
        road_wheel_unit_mesh(&kin),
        idler_unit_mesh(&kin),
        sprocket_unit_mesh(&kin),
        track_link_unit_mesh(&kin),
    ] {
        assert!(mesh.vertex_count() > 0 && mesh.triangle_count() > 0);
        assert!(mesh.vertices().iter().all(|v| v.position.is_finite()));
    }
}

#[test]
fn t54_top_track_run_sags_without_return_rollers() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    let mut top_links: Vec<Vec3> = placements
        .iter()
        .filter(|p| p.part == GearPart::Link)
        .map(|p| p.transform.w_axis.truncate())
        .filter(|p| p.x > 0.0 && p.y > kin.cy)
        .collect();
    top_links.sort_by(|a, b| a.z.total_cmp(&b.z));

    let mid = top_links
        .iter()
        .min_by(|a, b| a.z.abs().total_cmp(&b.z.abs()))
        .expect("top link near track middle");
    let ends_y = top_links
        .iter()
        .filter(|p| p.z.abs() > kin.half_run * 0.75)
        .map(|p| p.y)
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        ends_y - mid.y >= 0.04,
        "T-54 has no return rollers, so the top run should visibly sag: ends={ends_y:.3}, mid={:.3}",
        mid.y
    );
}

#[test]
fn t54_track_link_mesh_has_omsh_plate_horns_and_pin_cues() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let mesh = track_link_unit_mesh(&kin);
    let bounds = mesh.bounds().expect("link bounds");

    assert!(
        mesh.triangle_count() >= 48,
        "OMSh-style link needs plate, guide horns, and pin cues; got {} triangles",
        mesh.triangle_count()
    );
    assert!(mesh.vertices().iter().all(|v| v.material == MaterialRole::TrackMetal));
    assert!(
        bounds.max.x - bounds.min.x > kin.link_half_width * 2.4,
        "link plate should read as a wide metal shoe across the side-to-side track width"
    );
    assert!(
        bounds.min.y < -0.10,
        "inner guide horns should protrude below the flat shoe, bounds={bounds:?}"
    );
    assert!(
        bounds.max.y < 0.08,
        "bottom run should stay flattened instead of becoming a round rubber tube"
    );
}

#[test]
fn t54_sprocket_is_visibly_toothed_while_idler_is_smooth() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let idler = idler_unit_mesh(&kin);
    let sprocket = sprocket_unit_mesh(&kin);

    assert!(
        sprocket.triangle_count() > idler.triangle_count(),
        "rear drive sprocket should carry tooth geometry beyond the smooth front idler"
    );
    assert!(
        sprocket.bounds().expect("sprocket bounds").max.y
            > idler.bounds().expect("idler bounds").max.y,
        "sprocket teeth should extend past the smooth idler radius"
    );
}
