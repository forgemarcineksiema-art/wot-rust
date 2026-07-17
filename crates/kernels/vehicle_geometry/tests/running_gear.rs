//! Locking tests for the animatable running gear: placement counts, wheel spin, belt wrap,
//! per-side independence, and determinism.

use game_core::VehicleKind;
use glam::{Mat4, Vec3};
use vehicle_geometry::{
    GearPart, GeometryMesh, MaterialRole, RunningGearKinematics, idler_unit_mesh,
    road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh, track_link_unit_mesh,
};

fn t54() -> RunningGearKinematics {
    RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has blueprint running gear")
}

fn count(placements: &[vehicle_geometry::GearPlacement], part: GearPart) -> usize {
    placements.iter().filter(|p| p.part == part).count()
}

fn mats_close(a: Mat4, b: Mat4) -> bool {
    a.to_cols_array().iter().zip(b.to_cols_array()).all(|(x, y)| (x - y).abs() < 1.0e-3)
}

fn transformed_min_y(mesh: &GeometryMesh, transform: Mat4) -> f32 {
    mesh.vertices()
        .iter()
        .map(|vertex| transform.transform_point3(vertex.position).y)
        .fold(f32::INFINITY, f32::min)
}

fn rounded_axis_values(
    mesh: &GeometryMesh,
    material: MaterialRole,
) -> std::collections::BTreeSet<i32> {
    mesh.vertices()
        .iter()
        .filter(|vertex| vertex.material == material)
        .map(|vertex| (vertex.position.x * 1000.0).round() as i32)
        .collect()
}

/// Every playable vehicle animates its running gear — blueprint tanks from their blueprint's
/// track, the German fleet from the authored legacy-track table. Only the test-only prototype
/// medium keeps fused static gear.
#[test]
fn every_playable_vehicle_has_animated_gear() {
    for kind in VehicleKind::PLAYABLE {
        let kin = RunningGearKinematics::for_vehicle(kind)
            .unwrap_or_else(|| panic!("{kind:?} must animate its running gear"));
        assert!(kin.wheel_zs.len() >= 5, "{kind:?} fields a full wheel run");
        assert!(kin.link_count() >= 40, "{kind:?} belt reads as a segmented band");
        assert!(kin.wheel_radius > 0.2 && kin.wheel_radius < 0.6, "{kind:?} sane wheel");
    }
    assert!(RunningGearKinematics::for_vehicle(VehicleKind::PrototypeMedium).is_none());
}

/// The legacy hand-authored track table is gone with the last legacy vehicle: every animated
/// vehicle's gear now comes from its blueprint, and only the test-only prototype keeps fused
/// static gear.
#[test]
fn the_whole_animated_fleet_rides_blueprint_gear() {
    for kind in VehicleKind::PLAYABLE {
        let has_blueprint = game_core::VehicleBlueprint::for_vehicle(kind).is_some();
        assert!(has_blueprint, "{kind:?} must carry a blueprint track");
        assert!(RunningGearKinematics::for_vehicle(kind).is_some(), "{kind:?} animates");
    }
    assert!(RunningGearKinematics::for_vehicle(VehicleKind::PrototypeMedium).is_none());
}

#[test]
fn placements_cover_both_sides_for_every_part() {
    let kin = t54();
    let placements = running_gear_placements(&kin, 0.0, 0.0);

    // Two sides: road wheels = wheel_zs * 2, plus front idler and rear drive sprocket per side.
    assert_eq!(count(&placements, GearPart::RoadWheel), kin.wheel_zs.len() * 2);
    assert_eq!(count(&placements, GearPart::Idler), 2);
    assert_eq!(count(&placements, GearPart::Sprocket), 2);
    assert_eq!(count(&placements, GearPart::Link), kin.link_count() * 2);
}

#[test]
fn the_is3_top_run_rides_three_return_rollers_per_side() {
    // Return rollers are the IS family's look: six 550 mm wheels below, a TAUT top run carried
    // on three small rollers per side. The T-54 family keeps its rollerless, wheel-riding run.
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::IS3).expect("IS-3 gear");
    assert_eq!(kin.roller_zs.len(), 3, "three rollers per side");
    let track =
        game_core::VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("blueprint").track;
    assert!(
        (kin.roller_y + kin.roller_radius - track.top_y).abs() < 1.0e-6,
        "the roller TOP carries the belt's top run"
    );
    assert!(kin.top_sag_m < 0.02, "a rollered top run stays taut, got {}", kin.top_sag_m);
    assert_eq!(kin.wheel_zs.len(), 6, "six road wheels per side");

    let placements = running_gear_placements(&kin, 0.0, 0.0);
    assert_eq!(count(&placements, GearPart::ReturnRoller), 6, "three rollers, both sides");
    // Rollers spin with the belt: a rolled phase changes their rotation.
    let rolled = running_gear_placements(&kin, 1.0, 1.0);
    for (rest, moved) in placements
        .iter()
        .zip(&rolled)
        .filter(|(placement, _)| placement.part == GearPart::ReturnRoller)
    {
        assert!(!mats_close(rest.transform, moved.transform), "rollers must spin with the belt");
    }

    let t54 = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    assert!(t54.roller_zs.is_empty(), "the T-54 family stays rollerless");
    assert_eq!(count(&running_gear_placements(&t54, 0.0, 0.0), GearPart::ReturnRoller), 0);
}

#[test]
fn a_full_wheel_circumference_of_travel_returns_the_wheel_to_its_pose() {
    let kin = t54();
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
    let kin = t54();
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
    let kin = t54();
    let rest = running_gear_placements(&kin, 0.0, 0.0);
    let looped = running_gear_placements(&kin, kin.belt_length(), kin.belt_length());

    // Advancing the links by exactly one loop length lands them back on the same positions.
    for (a, b) in rest.iter().zip(&looped).filter(|(a, _)| a.part == GearPart::Link) {
        assert!(mats_close(a.transform, b.transform), "links must wrap continuously over the loop");
    }
}

#[test]
fn the_two_tracks_run_independently() {
    let kin = t54();
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
    let kin = t54();
    assert_eq!(running_gear_placements(&kin, 3.3, -1.7), running_gear_placements(&kin, 3.3, -1.7));
}

#[test]
fn unit_meshes_are_finite_and_non_empty() {
    let kin = t54();
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
fn t54_uses_historical_ninety_track_links_per_side() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let link_half_length = (kin.belt_length() / kin.link_count() as f32) * 0.47;

    assert_eq!(kin.link_count(), 90, "T-54 should render 90 metal links per track");
    assert!(
        link_half_length < 0.070,
        "90-link track needs short shoes that can follow end-wheel arcs; got half length {:.3}",
        link_half_length
    );
}

#[test]
fn t54_end_wrap_links_are_dense_around_idler_and_sprocket() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let placements = running_gear_placements(&kin, 0.0, 0.0);

    // The idler/sprocket sit beyond the road wheels on raised axles; the window covers the wrap
    // arc plus the tangent ramp feeding it, where the short links must read as a curved run.
    for (name, center_z) in [("sprocket", -kin.end_cz), ("idler", kin.end_cz)] {
        let wrapped = placements
            .iter()
            .filter(|placement| placement.part == GearPart::Link)
            .filter(|placement| placement.transform.w_axis.x > 0.0)
            .filter(|placement| {
                (placement.transform.w_axis.z - center_z).abs() < kin.end_radius + 0.15
            })
            .count();
        assert!(
            wrapped >= 10,
            "{name} wrap needs enough short links to read as a curved track run, got {wrapped}"
        );
    }
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
        bounds.min.y < -0.035,
        "inner guide horns should be visible below the flat shoe, bounds={bounds:?}"
    );
    assert!(
        bounds.min.y > -0.075,
        "guide horns must stay shallow; deep comb teeth hang through the top run and wheels, bounds={bounds:?}"
    );
    assert!(
        bounds.max.y < 0.08,
        "bottom run should stay flattened instead of becoming a round rubber tube"
    );
}

#[test]
fn t54_top_track_links_clear_the_road_wheel_tops() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let link = track_link_unit_mesh(&kin);
    let top_clearance_y = kin.cy + kin.wheel_radius - 0.075;
    let min_top_link_y = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|placement| placement.part == GearPart::Link)
        .filter(|placement| placement.transform.w_axis.x > 0.0)
        .filter(|placement| placement.transform.w_axis.y > kin.cy + kin.wheel_radius * 0.75)
        .filter(|placement| placement.transform.w_axis.z.abs() < kin.half_run * 0.70)
        .map(|placement| transformed_min_y(&link, placement.transform))
        .fold(f32::INFINITY, f32::min);

    assert!(
        min_top_link_y >= top_clearance_y,
        "top-run links must not hang down into the road wheels: min={min_top_link_y:.3}, clearance={top_clearance_y:.3}"
    );
}

#[test]
fn t54_track_shoes_ride_over_the_wheel_plane() {
    // The belt wraps the road wheels, so each shoe must straddle the wheel plane (the wheel runs
    // under the shoe), not float as a separate ribbon outboard of the running gear.
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let link = track_link_unit_mesh(&kin).bounds().expect("link bounds");

    let right_link_inner_x = kin.link_x + link.min.x;
    let right_link_outer_x = kin.link_x + link.max.x;
    assert!(
        right_link_inner_x < kin.wheel_x && kin.wheel_x < right_link_outer_x,
        "track shoe must straddle the wheel plane: link x=[{right_link_inner_x:.3}, {right_link_outer_x:.3}], wheel x={:.3}",
        kin.wheel_x
    );
}

#[test]
fn t54_road_wheels_have_metal_faces_and_rubber_tires() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let wheel = road_wheel_unit_mesh(&kin);

    assert!(wheel.vertices().iter().any(|vertex| vertex.material == MaterialRole::Rubber));
    assert!(
        wheel.vertices().iter().any(|vertex| vertex.material == MaterialRole::TrackMetal),
        "T-54 road wheels need visible metal discs/hubs, not a single black rubber cylinder"
    );
    assert!(
        wheel.triangle_count() > kin.segments * 4,
        "road wheel mesh should carry tire plus disc/hub detail; got {} triangles",
        wheel.triangle_count()
    );
}

#[test]
fn t54_road_wheel_reads_as_a_double_wheel_pair() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let wheel = road_wheel_unit_mesh(&kin);
    let rubber_xs = rounded_axis_values(&wheel, MaterialRole::Rubber);

    assert!(
        rubber_xs.len() >= 4,
        "double road wheels need two separated rubber tires, got x bands {rubber_xs:?}"
    );
}

#[test]
fn t54_road_wheel_face_shows_steel_not_a_solid_rubber_disc() {
    // The steel disc must fill most of the wheel face while the rubber stays at the rim, so the
    // wheel reads as a steel road wheel with a tire — not one black rubber ball.
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let wheel = road_wheel_unit_mesh(&kin);
    let max_radius = |material: MaterialRole| {
        wheel
            .vertices()
            .iter()
            .filter(|v| v.material == material)
            .map(|v| (v.position.y * v.position.y + v.position.z * v.position.z).sqrt())
            .fold(0.0_f32, f32::max)
    };

    assert!(
        max_radius(MaterialRole::TrackMetal) >= kin.wheel_radius * 0.7,
        "steel disc should fill most of the wheel face, not sit as a tiny hub"
    );
    assert!(
        max_radius(MaterialRole::Rubber) >= kin.wheel_radius * 0.97,
        "rubber tire must ride at the wheel rim"
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
    // The honest-width contract (W1 PR-T1.2): the toothed rings live INSIDE the belt band —
    // the sprocket's outermost face lands on the blueprint's `outer_x` (the documented width
    // over tracks), never proud of it — and the teeth are radially capped under the shoe
    // plates, so they show in the gaps between shoes instead of clipping through them.
    let bounds = sprocket.bounds().expect("sprocket bounds");
    assert!(
        bounds.max.x <= kin.band_half_width + 1.0e-4,
        "no sprocket vertex may stand proud of the belt band (outer_x), got {}",
        bounds.max.x
    );
    assert!(
        bounds.max.x >= kin.band_half_width - 0.06,
        "the tooth rings should reach the band's outer face, got {}",
        bounds.max.x
    );
    assert!(
        bounds.max.y > kin.end_radius * 0.85,
        "the teeth must still stand visibly proud of the carrier drum, got {}",
        bounds.max.y
    );
    for vertex in sprocket.vertices() {
        let radial =
            (vertex.position.y * vertex.position.y + vertex.position.z * vertex.position.z).sqrt();
        assert!(
            radial <= kin.end_radius - 0.010 + 1.0e-4,
            "a tooth must stay under the shoe plates riding the wrap: {:?}",
            vertex.position
        );
    }
    // The idler is a smooth wheel: its silhouette stays a round rim with no radial spikes, so the
    // front of the track is visibly plain against the rear sprocket's teeth.
    let idler_bounds = idler.bounds().expect("idler bounds");
    assert!(
        idler_bounds.max.y <= kin.end_radius + 0.010
            && idler_bounds.max.z <= kin.end_radius + 0.010,
        "front idler must read as a smooth round wheel, not a toothed ring"
    );
}
