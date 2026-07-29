//! Locking tests for the animatable running gear: placement counts, wheel spin, belt wrap,
//! per-side independence, and determinism.

use game_core::VehicleKind;
use glam::{Mat4, Vec3};
use vehicle_geometry::{
    GearPart, GeometryMesh, MaterialRole, RunningGearKinematics, SmoothingGroup, idler_unit_mesh,
    road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh, swing_arm_unit_mesh,
    track_link_unit_mesh,
};

#[test]
fn tiger_ii_dish_face_normals_stay_axial_across_the_whole_plate() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::TigerII).expect("Tiger II gear");
    let wheel = road_wheel_unit_mesh(&kin);
    let body_half = kin.wheel_half_width * 0.92;
    let face: Vec<_> = wheel
        .vertices()
        .iter()
        .filter(|vertex| vertex.smoothing == SmoothingGroup(5))
        .filter(|vertex| {
            let radius = (vertex.position.y * vertex.position.y
                + vertex.position.z * vertex.position.z)
                .sqrt();
            radius >= kin.wheel_radius * 0.15 && radius <= kin.wheel_radius * 0.89
        })
        .filter(|vertex| vertex.position.x > 0.0 && vertex.position.x <= body_half * 1.05)
        .collect();

    assert!(face.len() >= kin.segments, "test must cover the visible conical dish");
    let axial = face.iter().filter(|vertex| vertex.normal.x > 0.90).count();
    assert!(
        axial >= kin.segments.max(22) * 2,
        "both rings of the shallow front dish must stay plate-like; got {axial} axial normals"
    );
}

/// A tyre is a band pressed onto the rim, so it is CLOSED on its inner face. An open revolve
/// would show its hollow interior through the gap between the discs.
///
/// The Soviet twin-disc wheels are two separate bands with a real axial slot between them (the
/// horn's road), so each band answers for itself — a single triangle spanning the whole wheel
/// width, which this test used to demand, is exactly what a twin-tyre wheel must NOT have.
#[test]
fn every_road_wheel_tyre_band_is_closed_on_its_inner_face() {
    for (kind, material) in [
        (VehicleKind::TigerII, MaterialRole::TrackMetal),
        (VehicleKind::Centurion, MaterialRole::Rubber),
        (VehicleKind::T54_1951, MaterialRole::Rubber),
    ] {
        let kin = RunningGearKinematics::for_vehicle(kind).expect("vehicle has running gear");
        let wheel = road_wheel_unit_mesh(&kin);
        let expected_radius = kin.wheel_radius * 0.86;
        let gap_half = kin.tyre_gap_half();

        // Each band's own span: one band across the wheel when there is no slot, two when there
        // is. A band is closed when a triangle lies wholly on the inner wall and spans it.
        let bands: Vec<(f32, f32)> = if gap_half > 0.0 {
            vec![(-kin.wheel_half_width, -gap_half), (gap_half, kin.wheel_half_width)]
        } else {
            vec![(-kin.wheel_half_width, kin.wheel_half_width)]
        };
        for (x0, x1) in bands {
            let closed = wheel.indices().chunks_exact(3).any(|triangle| {
                let vertices: Vec<_> =
                    triangle.iter().map(|&index| &wheel.vertices()[index as usize]).collect();
                let on_inner_wall = vertices.iter().all(|vertex| {
                    let radius = vertex.position.y.hypot(vertex.position.z);
                    vertex.material == material && (radius - expected_radius).abs() <= 1.0e-4
                });
                let min_x = vertices.iter().map(|v| v.position.x).fold(f32::INFINITY, f32::min);
                let max_x = vertices.iter().map(|v| v.position.x).fold(f32::NEG_INFINITY, f32::max);
                let span = x1 - x0;
                on_inner_wall && min_x <= x0 + span * 0.1 && max_x >= x1 - span * 0.1
            });
            assert!(
                closed,
                "{kind:?}: the tyre band from {x0:.3} to {x1:.3} has no inner wall — an open                  revolve ring shows its hollow interior, and on a twin-disc wheel it shows it                  through the slot the guide horn rides in"
            );
        }

        // And the slot is REAL: on a twin-disc wheel no rubber crosses the centreline.
        if gap_half > 0.0 {
            let rubber_in_the_slot = wheel.vertices().iter().any(|vertex| {
                vertex.material == MaterialRole::Rubber && vertex.position.x.abs() < gap_half * 0.9
            });
            assert!(
                !rubber_in_the_slot,
                "{kind:?}: rubber crosses the slot between the tyres — that is a grooved single                  tyre pretending to be a pair, and the guide horn has nowhere to go"
            );
        }
    }
}

#[test]
fn dished_wheel_faces_meet_the_tire_lip_instead_of_floating_deep_inside_it() {
    for kind in [VehicleKind::TigerII, VehicleKind::Centurion] {
        let kin = RunningGearKinematics::for_vehicle(kind).expect("vehicle has running gear");
        let wheel = road_wheel_unit_mesh(&kin);
        let rim_radius = kin.wheel_radius * 0.88;
        let front_rim_x = wheel
            .vertices()
            .iter()
            .filter(|vertex| vertex.smoothing == SmoothingGroup(5))
            .filter(|vertex| {
                let radius = vertex.position.y.hypot(vertex.position.z);
                (radius - rim_radius).abs() <= 1.0e-4
            })
            .map(|vertex| vertex.position.x)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            front_rim_x >= kin.wheel_half_width * 0.85,
            "{kind:?} dish rim x={front_rim_x:.3} sits too deep behind tire lip x={:.3}",
            kin.wheel_half_width
        );
    }
}

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

/// THE OMSh SHOE, CHECKED AGAINST WHAT AN OMSh SHOE IS.
///
/// The test this replaces promised "plate, horns and pin cues" and verified none of them. Its
/// depth assertion was satisfied by the BACKING SKIN — the anti-strobe band under the shoe — so
/// it passed for years on a link that had no guide horn at all, while its upper bound
/// (`min.y > -0.075`) actively forbade one. A test that forbids the part it is named after is
/// worse than no test.
///
/// What an OMSh link has (dossier "Part construction", S1b): one guide horn on every shoe from
/// September 1949, standing up into the gap between the road wheel's twin tyres; a barrel of
/// hinge eyes at each joint — the цевка the sprocket tooth bears on; a floating pin whose ends
/// stop about 30 mm inboard of the belt edges, so no pin heads show from outside; and stiffening
/// ribs on the ground face.
#[test]
fn t54_track_link_mesh_is_a_horned_omsh_shoe() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let mesh = track_link_unit_mesh(&kin);
    let bounds = mesh.bounds().expect("link bounds");
    assert!(mesh.vertices().iter().all(|v| v.material == MaterialRole::TrackMetal));

    // The plate spans the belt band: the documented width over tracks.
    assert!(
        bounds.max.x - bounds.min.x > kin.link_half_width * 2.4,
        "the shoe plate reads as a wide metal shoe across the track width"
    );

    // THE HORN. Narrow enough to enter the slot between the twin tyres, and proud enough to be
    // swallowed by it rather than to ride in a dent.
    let gap_half = kin.tyre_gap_half();
    assert!(gap_half > 0.0, "a Soviet twin-disc wheel has a slot for the horn to ride in");
    let horn_depth = mesh
        .vertices()
        .iter()
        .filter(|v| v.position.x.abs() <= gap_half)
        .map(|v| v.position.y)
        .fold(f32::INFINITY, f32::min);
    let shoulder_depth = mesh
        .vertices()
        .iter()
        .filter(|v| v.position.x.abs() > gap_half * 1.5)
        .map(|v| v.position.y)
        .fold(f32::INFINITY, f32::min);
    assert!(
        horn_depth < shoulder_depth - 0.020,
        "the horn must stand proud of everything beside it: horn {horn_depth:.3} vs shoulder \
         {shoulder_depth:.3}"
    );
    let slot_depth = kin.wheel_radius - kin.tyre_seat_radius();
    assert!(
        horn_depth >= shoulder_depth - slot_depth,
        "the horn must fit the slot it rides in ({slot_depth:.3} m deep), not bottom out on the \
         wheel: horn {horn_depth:.3}, shoulder {shoulder_depth:.3}"
    );
    let horn_width = mesh
        .vertices()
        .iter()
        .filter(|v| v.position.y < shoulder_depth - 0.010)
        .map(|v| v.position.x.abs())
        .fold(0.0_f32, f32::max);
    assert!(
        horn_width <= gap_half,
        "the horn must clear the tyres it runs between: {horn_width:.3} vs gap half {gap_half:.3}"
    );

    // THE PIN LINE. The pin floats and its ends stop inboard of the belt edges, so nothing of the
    // hinge reaches the outer face — a shoe showing pin heads is a different track.
    let joint_z = (kin.belt_length() / kin.link_count() as f32) * 0.47 * 0.80;
    let widest_at_joint = mesh
        .vertices()
        .iter()
        // Below the backing skin's face: at the joint, only the eye barrel is down there.
        .filter(|v| v.position.z.abs() > joint_z && v.position.y < -0.055)
        .map(|v| v.position.x.abs())
        .fold(0.0_f32, f32::max);
    assert!(
        widest_at_joint < kin.band_half_width - 0.010,
        "the hinge stops inboard of the belt edge ({widest_at_joint:.3} vs band \
         {:.3}) — the pin does not show",
        kin.band_half_width
    );
    assert!(
        widest_at_joint > kin.band_half_width * 0.5,
        "but the eye barrel still runs most of the width: {widest_at_joint:.3}"
    );

    // And the ground face stays a flattened shoe, not a round tube.
    assert!(bounds.max.y < 0.08, "the bottom run is a flat shoe, not a rubber tube");
}

#[test]
fn centurion_uses_three_horstmann_bogies_per_side() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::Centurion).expect("Centurion gear");
    assert_eq!(kin.suspension, game_core::SuspensionKind::Horstmann);
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    assert_eq!(count(&placements, GearPart::SwingArm), 6, "three paired bogies per side");

    let bounds = swing_arm_unit_mesh(&kin).bounds().expect("Horstmann bounds");
    assert!(bounds.max.z - bounds.min.z > 0.5, "bogie rocker spans a wheel pair");
    assert!(bounds.max.y - bounds.min.y > 0.3, "bogie carries a visible spring housing");
}

#[test]
fn t34_uses_one_christie_crank_per_wheel() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T34_85).expect("T-34 gear");
    assert_eq!(kin.suspension, game_core::SuspensionKind::Christie);
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    assert_eq!(count(&placements, GearPart::SwingArm), 10, "five Christie cranks per side");

    let bounds = swing_arm_unit_mesh(&kin).bounds().expect("Christie bounds");
    assert!(bounds.max.y - bounds.min.y > 0.2, "crank rises steeply into the hull side");
    assert!(bounds.max.z - bounds.min.z < 0.4, "crank is not a long paired bogie beam");
}

#[test]
fn every_animated_track_link_carries_an_overlapping_backing_skin() {
    for kind in VehicleKind::PLAYABLE {
        let kin = RunningGearKinematics::for_vehicle(kind).expect("playable vehicle gear");
        let bounds = track_link_unit_mesh(&kin).bounds().expect("link bounds");
        let pitch = kin.belt_length() / kin.link_count() as f32;
        assert!(
            bounds.max.z - bounds.min.z > pitch,
            "{kind:?} backing must overlap neighbouring links around wraps: span {:.3}, pitch {pitch:.3}",
            bounds.max.z - bounds.min.z,
        );
    }
}

/// The top run lies ON the road wheels, so nothing on a shoe may hang into one — with exactly
/// one exception, which is the whole point of the shoe: the guide horn drops into the SLOT
/// between the wheel's twin tyres. That is what keeps the belt on.
///
/// So the rule is measured in two pieces. Everything outside the slot clears the tread; the horn
/// clears the seat the tyres are pressed onto. Before the horn existed the first piece was the
/// whole test, and it quietly forbade the second.
#[test]
fn t54_top_track_links_ride_the_wheels_with_only_the_horn_in_the_slot() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let link = track_link_unit_mesh(&kin);
    let gap_half = kin.tyre_gap_half();
    let tread_clearance_y = kin.cy + kin.wheel_radius - 0.075;
    let seat_clearance_y = tread_clearance_y - (kin.wheel_radius - kin.tyre_seat_radius());

    let top_links: Vec<_> = running_gear_placements(&kin, 0.0, 0.0)
        .into_iter()
        .filter(|placement| placement.part == GearPart::Link)
        .filter(|placement| placement.transform.w_axis.x > 0.0)
        .filter(|placement| placement.transform.w_axis.y > kin.cy + kin.wheel_radius * 0.75)
        .filter(|placement| placement.transform.w_axis.z.abs() < kin.half_run * 0.70)
        .collect();
    assert!(!top_links.is_empty(), "the top run must have links on it");

    let (mut shoulder_min, mut horn_min) = (f32::INFINITY, f32::INFINITY);
    for placement in &top_links {
        let centre_x = placement.transform.w_axis.x;
        for vertex in link.vertices() {
            let world = placement.transform.transform_point3(vertex.position);
            // Inside the slot in the wheel's own frame: the belt plane is the wheel plane.
            if (world.x - centre_x).abs() <= gap_half {
                horn_min = horn_min.min(world.y);
            } else {
                shoulder_min = shoulder_min.min(world.y);
            }
        }
    }

    assert!(
        shoulder_min >= tread_clearance_y,
        "the shoe body must ride ON the tread: min={shoulder_min:.3}, \
         clearance={tread_clearance_y:.3}"
    );
    assert!(
        horn_min >= seat_clearance_y,
        "the horn must stay inside the slot, not grind on the wheel: min={horn_min:.3}, \
         seat={seat_clearance_y:.3}"
    );
    assert!(
        horn_min < shoulder_min,
        "and it must actually be IN the slot: horn {horn_min:.3} vs shoe {shoulder_min:.3}"
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

/// THE DRIVE SPROCKET ACTUALLY DRIVES.
///
/// Its teeth used to stop 32 mm short of the belt line: decoration passing near a track it never
/// touched, on the one wheel whose entire job is to push it. Nothing caught that, because no test
/// asked the two parts about each other.
///
/// What a tooth must do is reach the surface it bears on and stop there. On an OMSh belt that
/// surface is the barrel of hinge eyes on the wheel side of each shoe — the цевка — and stopping
/// at it is also what keeps the tooth out of the shoe plate, which rides further out on the wrap.
/// Both parts read the stand-off from `hinge_eye_offset`, so this is one number, checked.
#[test]
fn sprocket_teeth_reach_the_hinge_eyes_they_bear_on() {
    for kind in VehicleKind::PLAYABLE {
        let kin = RunningGearKinematics::for_vehicle(kind).expect("gear");
        let mesh = sprocket_unit_mesh(&kin);
        let reach = mesh
            .vertices()
            .iter()
            .map(|v| v.position.y.hypot(v.position.z))
            .fold(0.0_f32, f32::max);

        // Where the belt runs on the wrap, and where the eye barrel sits under it.
        let belt_r = kin.end_radius + 0.02;
        let eye_r = belt_r - kin.hinge_eye_offset();

        assert!(
            reach >= eye_r,
            "{kind:?}: the teeth reach {reach:.3} m but the hinge eyes they drive are at              {eye_r:.3} m — this sprocket is not touching the track"
        );
        assert!(
            reach <= belt_r - kin.hinge_eye_offset() * 0.30,
            "{kind:?}: the teeth reach {reach:.3} m, past the eyes and into the shoe plate at              {belt_r:.3} m — a tooth through the shoe is not engagement"
        );
    }
}

/// The tooth count is not a style choice: a tooth must meet a link, so it is the number of link
/// pitches around the wrap circle. Recording what that resolves to also records a dimensional
/// deviation the register already owns.
///
/// The T-54's documented sprocket has **13 teeth per ring**; 14 is the modernised T-55 / Obj. 167
/// wheel. Ours derives 14, and it is right to: our belt pitch is 142 mm against the documented
/// 137, and our wrap radius 0.32 m against a 0.286 m pitch circle. The count follows the belt, so
/// the count cannot be fixed here — it falls out when the track dimensions land (M9 / PR-18).
/// Asserting 13 now would put a sprocket on the tank that its own belt does not fit.
#[test]
fn the_t54_tooth_count_records_the_belt_it_has_to_mesh_with() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("gear");
    let pitch = kin.belt_length() / kin.link_count() as f32;
    let wrap_r = kin.end_radius + 0.02;
    let teeth = ((std::f32::consts::TAU * wrap_r) / pitch).round() as usize;
    assert_eq!(
        teeth, 14,
        "the derived tooth count moved; if the belt pitch or wrap radius changed, check it          against the documented 13 and update this record"
    );
    println!(
        "SPROCKET DEBT T54: {teeth} teeth per ring from a {:.4} m pitch on a {wrap_r:.3} m wrap          (documented: 13 teeth, 0.137 m pitch, 0.286 m pitch circle — M9/PR-18)",
        pitch
    );
}

/// The idler is the TENSIONER. Its eccentric crank is the part that makes a thrown track a
/// repair, and it is also how you tell the idler end of the tank from the drive end at a glance.
#[test]
fn the_idler_carries_its_tension_crank() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("gear");
    let mesh = idler_unit_mesh(&kin);
    let inboard_of_the_wheel =
        mesh.vertices().iter().filter(|v| v.position.x < -kin.wheel_half_width * 1.2).count();
    assert!(
        inboard_of_the_wheel > 0,
        "the tension crank must stand inboard of the idler disc, where the hull bearing is"
    );
    let below_the_axle = mesh
        .vertices()
        .iter()
        .filter(|v| v.position.x < -kin.wheel_half_width * 1.2)
        .map(|v| v.position.z.hypot(v.position.y))
        .fold(0.0_f32, f32::max);
    assert!(
        below_the_axle > kin.end_radius * 0.35,
        "the crank arm must reach out to its bearing, not sit on the axle: {below_the_axle:.3}"
    );

    // And its tyres are STEEL, not rubber (dossier: 510 mm cast wheel, steel tyres).
    assert!(
        mesh.vertices().iter().all(|v| v.material == MaterialRole::TrackMetal),
        "the T-54 idler runs on steel tyres — rubber made it read as a second road wheel"
    );
}

/// THE SUSPENSION IS THE VEHICLE'S, NOT THE FLEET'S.
///
/// A trailing arm's reach and rise decide where a wheel sits when the suspension works and how
/// far it can travel — and they were two constants shared by every torsion-bar tank in the game,
/// from a 36-tonne T-54 to a 70-tonne Tiger II. A suspension is one of the few things a tank is
/// actually judged on. Its geometry belongs in the blueprint with everything else the vehicle is.
#[test]
fn every_swing_arm_reads_its_geometry_from_its_own_blueprint() {
    for kind in VehicleKind::PLAYABLE {
        let blueprint = game_core::VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        let kin = RunningGearKinematics::for_vehicle(kind).expect("gear");
        assert_eq!(
            kin.arm_reach,
            blueprint.track.arm_reach(),
            "{kind:?}: the arm must reach what its blueprint says"
        );
        assert_eq!(kin.arm_rise, blueprint.track.arm_rise());

        // And the number actually drives the mesh: an arm authored longer IS longer.
        let mut stretched = blueprint.track;
        stretched.arm_reach_m = Some(blueprint.track.arm_reach() * 1.5);
        let long = RunningGearKinematics::from_track(&stretched);
        // How far the arm reaches from its pivot, measured in the plane it swings in. A
        // bounding box is the wrong ruler here: the section rotates with the arm's own angle, so
        // a box can shrink at one end while the axle moves further out.
        let reach_from_pivot = |k: &RunningGearKinematics| {
            swing_arm_unit_mesh(k)
                .vertices()
                .iter()
                .map(|v| v.position.y.hypot(v.position.z))
                .fold(0.0_f32, f32::max)
        };
        if kin.suspension != game_core::SuspensionKind::Horstmann {
            assert!(
                reach_from_pivot(&long) > reach_from_pivot(&kin) + 0.03,
                "{kind:?}: authoring a longer arm must carry its axle further from the pivot                  ({:.3} vs {:.3})",
                reach_from_pivot(&long),
                reach_from_pivot(&kin)
            );
        }
    }
}

/// A swing arm is a FORGING, not a slab. Its section is an I — thick at the flanges, waisted in
/// the web — and it carries the torsion-bar hub at its pivot, which is the part that makes the
/// mechanism legible: the bar twists inside that boss.
#[test]
fn the_torsion_arm_is_an_i_section_with_a_bar_hub() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("gear");
    let mesh = swing_arm_unit_mesh(&kin);

    // The section is not constant: sample the arm's thickness along its own length and require
    // the flanges to stand proud of the web between them.
    let thickness_at = |t: f32| {
        let along = t * kin.arm_reach;
        mesh.vertices()
            .iter()
            .filter(|v| (v.position.z + along).abs() < 0.020)
            .map(|v| v.position.x.abs())
            .fold(0.0_f32, f32::max)
    };
    let flange = thickness_at(0.05_f32);
    let web = thickness_at(0.42_f32);
    assert!(
        flange > web + 0.004,
        "the arm must be thicker at its flanges than in its web: {flange:.3} vs {web:.3} — a \
         constant-thickness arm is a plate, not a forging"
    );

    // The torsion-bar hub stands INBOARD of the arm plate, where the bar runs across the floor.
    let inboard = mesh.vertices().iter().map(|v| v.position.x).fold(f32::INFINITY, f32::min);
    assert!(
        inboard < -0.070,
        "the torsion-bar boss must stand proud toward the hull: innermost {inboard:.3}"
    );
}
