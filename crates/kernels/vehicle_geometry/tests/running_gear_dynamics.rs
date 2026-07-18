//! Locking tests for the running gear's live dynamics: per-wheel travel, drive tension,
//! per-side sag (a thrown track hangs), and the single source of wheel stations shared with
//! the physics contact footprint. Split from `running_gear.rs` (file budget).

use game_core::VehicleKind;
use vehicle_geometry::{GearPart, RunningGearKinematics, running_gear_placements};

fn t54() -> RunningGearKinematics {
    RunningGearKinematics::for_vehicle(VehicleKind::T54_1951)
        .expect("T-54 has blueprint running gear")
}

#[test]
fn wheel_overlap_pulls_only_the_odd_row_inboard() {
    // Schachtellaufwerk plumbing: with an overlap authored, odd-indexed wheels ride an inner
    // row; even wheels and everything else stay put. Zero overlap is byte-for-byte today's
    // single file — the existing fleet's golden hashes never move.
    let mut kin = t54();
    let single_file = running_gear_placements(&kin, 0.0, 0.0);
    kin.wheel_overlap_dx = 0.20;
    let interleaved = running_gear_placements(&kin, 0.0, 0.0);

    let wheel_xs = |set: &[vehicle_geometry::GearPlacement]| -> Vec<f32> {
        set.iter()
            .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
            .map(|p| p.transform.w_axis.x)
            .collect()
    };
    let before = wheel_xs(&single_file);
    let after = wheel_xs(&interleaved);
    assert!(before.windows(2).all(|w| (w[0] - w[1]).abs() < 1.0e-6), "today: one file");
    // Odd axles ride the inner row and even axles stay on the outer row. One axle remains one
    // wheel unit; no invented third row is allowed to disappear into the lower hull tub.
    assert_eq!(after.len(), before.len());
    let outer = before[0];
    let has = |x: f32| after.iter().any(|a| (a - x).abs() < 1.0e-4);
    assert!(has(outer), "outer row survives");
    assert!(has(outer - 0.20), "odd axles ride the inner row");
    for a in &after {
        assert!(
            [outer, outer - 0.20].iter().any(|x| (a - x).abs() < 1.0e-4),
            "every disc sits on one of the two authored planes, got {a}"
        );
    }
}

#[test]
fn a_thrown_track_leaves_its_side_bare() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let whole = vehicle_geometry::running_gear_placements_dynamic(
        &kin,
        0.0,
        0.0,
        vehicle_geometry::GearDynamics::default(),
    );
    let broken = vehicle_geometry::running_gear_placements_dynamic(
        &kin,
        0.0,
        0.0,
        vehicle_geometry::GearDynamics { left_break_t: Some(0.5), ..Default::default() },
    );
    let left_links = |set: &[vehicle_geometry::GearPlacement]| {
        set.iter().filter(|p| p.part == GearPart::Link && p.transform.w_axis.x < 0.0).count()
    };
    let right_links = |set: &[vehicle_geometry::GearPlacement]| {
        set.iter().filter(|p| p.part == GearPart::Link && p.transform.w_axis.x > 0.0).count()
    };
    // Fizyczny Świat: the loop is OFF the wheels — the broken side draws no belt at all
    // (the shed ribbon on the ground carries the steel), the healthy side is untouched.
    assert!(left_links(&whole) > 0);
    assert_eq!(left_links(&broken), 0, "a thrown side rides bare wheels");
    assert_eq!(right_links(&whole), right_links(&broken));
}

#[test]
fn wheel_travel_moves_wheels_and_the_ground_run_follows() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let rest = running_gear_placements(&kin, 0.0, 0.0);
    // Drop the middle wheel into a dip on the right side only.
    let mut travel = [0.0_f32; 5];
    travel[2] = -0.06;
    let dynamics = vehicle_geometry::GearDynamics {
        left_travel: &[],
        right_travel: &travel,
        left_sag_scale: 1.0,
        right_sag_scale: 1.0,
        left_break_t: None,
        right_break_t: None,
    };
    let bumped = vehicle_geometry::running_gear_placements_dynamic(&kin, 0.0, 0.0, dynamics);

    // The right middle road wheel dropped by exactly the travel; the left stayed at rest.
    let wheel_y = |set: &[vehicle_geometry::GearPlacement], side: f32, z: f32| {
        set.iter()
            .filter(|p| p.part == GearPart::RoadWheel)
            .map(|p| p.transform.w_axis)
            .find(|w| w.x * side > 0.0 && (w.z - z).abs() < 0.01)
            .expect("wheel present")
            .y
    };
    let z_mid = kin.wheel_zs[2];
    assert!(
        (wheel_y(&bumped, 1.0, z_mid) - (wheel_y(&rest, 1.0, z_mid) - 0.06)).abs() < 1.0e-4,
        "the dipped wheel drops by its travel"
    );
    assert!(
        (wheel_y(&bumped, -1.0, z_mid) - wheel_y(&rest, -1.0, z_mid)).abs() < 1.0e-4,
        "the opposite side stays at rest"
    );

    // The ground-run links under that wheel follow it down; the top run does not move.
    let bottom_link_y = |set: &[vehicle_geometry::GearPlacement]| {
        set.iter()
            .filter(|p| p.part == GearPart::Link)
            .map(|p| p.transform.w_axis)
            .filter(|w| w.x > 0.0 && (w.z - z_mid).abs() < 0.3 && w.y < kin.cy)
            .map(|w| w.y)
            .fold(f32::INFINITY, f32::min)
    };
    assert!(
        bottom_link_y(&bumped) < bottom_link_y(&rest) - 0.03,
        "the ground run conforms to the dipped wheel"
    );
}

#[test]
fn drive_tension_tightens_the_top_run_and_slack_deepens_it() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let mid_top_y = |sag_scale: f32| {
        let dynamics = vehicle_geometry::GearDynamics {
            left_travel: &[],
            right_travel: &[],
            left_sag_scale: sag_scale,
            right_sag_scale: sag_scale,
            left_break_t: None,
            right_break_t: None,
        };
        vehicle_geometry::running_gear_placements_dynamic(&kin, 0.0, 0.0, dynamics)
            .iter()
            .filter(|p| p.part == GearPart::Link)
            .map(|p| p.transform.w_axis)
            .filter(|w| w.x > 0.0 && w.y > kin.cy && w.z.abs() < 0.4)
            .map(|w| w.y)
            .fold(f32::INFINITY, f32::min)
    };
    let driven = mid_top_y(0.55);
    let rest = mid_top_y(1.0);
    let braking = mid_top_y(1.5);
    assert!(driven > rest + 0.01, "a driven track pulls its top run tight");
    assert!(braking < rest - 0.01, "a braking track lets the top run hang");
}

#[test]
fn rendered_wheels_and_physics_footprint_share_one_set_of_stations() {
    // The wheels the player sees must be the wheels the hull rides on: the rendered running
    // gear and the physics contact footprint both read TrackShape::wheel_stations.
    for kind in [VehicleKind::T54_1951, VehicleKind::T54_1951] {
        let kin = RunningGearKinematics::for_vehicle(kind).expect("blueprint running gear");
        let footprint = game_core::ContactFootprint::for_vehicle(kind);
        assert_eq!(kin.wheel_zs.as_slice(), footprint.station_zs(), "{kind:?} stations diverged");
        assert!((kin.wheel_radius - footprint.wheel_radius).abs() < 1.0e-6);
        assert!((kin.wheel_x - footprint.half_gauge_x).abs() < 1.0e-6);
    }
}

#[test]
fn a_thrown_track_hangs_its_side_deep_while_the_healthy_side_keeps_tension() {
    let kin = t54();
    let placements = vehicle_geometry::running_gear_placements_dynamic(
        &kin,
        0.0,
        0.0,
        vehicle_geometry::GearDynamics {
            left_travel: &[],
            right_travel: &[],
            left_sag_scale: 2.2, // thrown: dead slack
            right_sag_scale: 1.0,
            left_break_t: None,
            right_break_t: None,
        },
    );
    let mid_top_y = |side: f32| {
        placements
            .iter()
            .filter(|p| p.part == GearPart::Link)
            .map(|p| p.transform.w_axis)
            .filter(|w| w.x * side > 0.0 && w.y > kin.cy && w.z.abs() < 0.4)
            .map(|w| w.y)
            .fold(f32::INFINITY, f32::min)
    };
    let left = mid_top_y(-1.0);
    let right = mid_top_y(1.0);
    assert!(left < right - 0.02, "thrown left must hang deeper: left {left}, right {right}");
}

/// Thrown-track phase 2: the fresh remnant drapes over the sprocket's wrap (top link seated at
/// the wrap's crown), the chain is link-to-link continuous, and a full slide takes every link
/// to the ground — the remnant is transient by construction.
#[test]
fn the_hanging_remnant_drapes_the_sprocket_and_slides_off() {
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 gear");
    let fresh = vehicle_geometry::thrown_remnant_placements(&kin, -1.0, 0.0);
    assert!(!fresh.is_empty(), "a fresh throw leaves a hanging remnant");
    let pitch = kin.belt_length() / kin.link_count() as f32;
    let positions: Vec<glam::Vec3> = fresh.iter().map(|p| p.transform.w_axis.truncate()).collect();
    for pair in positions.windows(2) {
        assert!(
            (pair[1] - pair[0]).length() <= pitch * 1.1,
            "the remnant is a chain, not scattered links"
        );
    }
    // The top link sits at the crown of the front wrap.
    let crown_y = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(crown_y > kin.end_cy, "the chain drapes OVER the sprocket: crown y {crown_y}");
    // Everything hangs on the requested side.
    assert!(positions.iter().all(|p| p.x < 0.0), "a left remnant hangs on the left");

    // A long slide takes the whole chain to the ground: nothing left to draw.
    let slid = vehicle_geometry::thrown_remnant_placements(&kin, -1.0, 10.0);
    assert!(slid.is_empty(), "the slid-off remnant has joined the band on the field");
}
