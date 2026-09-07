//! Low solids are support (the one program's X4).
//!
//! A solid whose top is within the running gear's step of the hull's current support enters the
//! support envelope like a mound does — the hull crosses it with a tilt and a speed loss — and
//! the separating-axis test lets the plan onto it by the same predicate. A taller solid is the
//! wall it always was.

use game_core::{ContactFootprint, HullPlan, TankSpec, VehicleKind};
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankFootprint, TankKinematicState,
    TankWorldObstacles, footprint_blocked_by_cover, step_solids_near,
    step_tank_on_world_with_tanks,
};
use terrain::{HeightMap, StaticCoverKind, StaticCoverObject};

const DT: f32 = 1.0 / 60.0;
const DRIVE: TankControlInput = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };

fn solid(kind: StaticCoverKind, height_m: f32, half_z: f32) -> StaticCoverObject {
    StaticCoverObject {
        id: "solid".into(),
        name: "solid".into(),
        kind,
        center: [60.0, height_m * 0.5, 60.0],
        half_extents_m: [8.0, height_m * 0.5, half_z],
        yaw_rad: 0.0,
    }
}

/// Drive the benchmark T-54 north from `z = 40` at the solid on `z = 60` and report the peak
/// ride height, the steepest nose-up pitch, and the final hull centre z.
fn run_at(cover: &[StaticCoverObject], ticks: usize) -> (f32, f32, f32) {
    let spec = TankSpec::t54_1951();
    let settings = TankControllerSettings::from_spec(&spec);
    let running_gear = ContactFootprint::for_vehicle(VehicleKind::T54_1951);
    let hull = TankFootprint::from_plan(spec.hull_plan());
    let map = HeightMap::flat(121, 121, 1.0, 0.0).expect("flat test terrain");
    let mut state =
        TankKinematicState { position: Vec3::new(60.0, 0.0, 40.0), ..Default::default() };
    let (mut peak_y, mut steepest) = (f32::MIN, 0.0_f32);
    for _ in 0..ticks {
        step_tank_on_world_with_tanks(
            &mut state,
            DRIVE,
            &settings,
            Some(&map),
            TankWorldObstacles::new(cover, hull),
            Some(&running_gear),
            DT,
        );
        peak_y = peak_y.max(state.position.y);
        steepest = steepest.max(state.pitch_rad);
    }
    (peak_y, steepest, state.position.z)
}

/// The row's lock: the parapet (a 0.6 m `LowWall`, a metre thick) is crossed with a tilt and a
/// speed loss. The hull rides up onto it — the support envelope carries it — and comes down the
/// far side having covered less ground than the same run over bare dirt.
#[test]
fn a_low_wall_within_the_step_is_crossed_with_a_tilt_and_a_speed_loss() {
    let parapet = solid(StaticCoverKind::LowWall, 0.6, 0.5);
    let (peak_y, steepest, crossed_to) = run_at(std::slice::from_ref(&parapet), 300);
    let (_, _, flat_to) = run_at(&[], 300);

    assert!(crossed_to > 62.0, "the hull must be past the parapet, got z {crossed_to}");
    assert!(peak_y > 0.45, "the hull must ride UP onto the parapet, peaked at {peak_y} m");
    assert!(steepest > 0.05, "the crossing must tilt the hull, steepest pitch {steepest} rad");
    assert!(
        crossed_to < flat_to - 0.15,
        "the crossing must cost ground: {crossed_to} over the parapet vs {flat_to} on dirt"
    );
}

/// ...and the churchyard wall (1.6 m of `StoneWall`) is not: taller than the step, it holds the
/// hull at its face exactly as before, and the hull never rises.
#[test]
fn a_wall_taller_than_the_step_still_holds_the_hull_at_its_face() {
    let churchyard = solid(StaticCoverKind::StoneWall, 1.6, 0.4);
    let (peak_y, _, stopped_at) = run_at(std::slice::from_ref(&churchyard), 300);
    let nose = stopped_at + HullPlan::for_vehicle(VehicleKind::T54_1951).half_length_m;
    assert!(nose <= 60.0 - 0.4 + 0.02, "the nose must stop at the wall face, got {nose}");
    assert!(peak_y < 0.05, "nothing lifts a hull over a wall it cannot step, got {peak_y}");
}

/// The step is measured from the CURRENT support, and the two readers agree: a wall the SAT
/// blocks from the street is one it lets the plan onto from half a metre up — and the gather
/// hands that same wall to the support envelope from there, and not from the street.
#[test]
fn the_step_is_measured_from_the_current_support_by_both_readers() {
    let wall = solid(StaticCoverKind::LowWall, 1.2, 0.5);
    let walls = std::slice::from_ref(&wall);
    let hull = TankFootprint::from_plan(HullPlan::for_vehicle(VehicleKind::T54_1951));
    assert!(hull.step_m < 1.2 && hull.step_m + 0.5 > 1.2, "the fixture needs 1.2 m between");

    let street = Vec3::new(60.0, 0.0, 57.0);
    let raised = Vec3::new(60.0, 0.5, 57.0);
    assert!(footprint_blocked_by_cover(street, 0.0, hull, walls), "from the street: a wall");
    assert!(step_solids_near(walls, street, hull, 5.0).is_empty(), "...and no step");
    assert!(!footprint_blocked_by_cover(raised, 0.0, hull, walls), "from 0.5 m up: a step");
    assert_eq!(step_solids_near(walls, raised, hull, 5.0).len(), 1, "...for the envelope too");
}
