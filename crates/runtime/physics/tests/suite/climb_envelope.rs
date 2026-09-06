//! CLIMBING IS A DISCIPLINE, AND THIS IS ITS ENVELOPE.
//!
//! Nobody wrote a climbing feature. It falls out of one decision: the grade the force model
//! resolves against is `forward_slope`, the slope **along the hull's heading**, so taking a face
//! obliquely presents a shallower grade than taking it square. A driver who approaches a bank at
//! sixty degrees is not exploiting a bug; they are paying for the climb in the distance and the
//! commitment the geometry asks for.
//!
//! The user's call (2026-08-06) is that this stays a discipline — a thing worth learning, with a
//! ceiling you can find and a technique that earns it. Which makes the envelope a CONTRACT, and
//! contracts get measured. Nothing guarded these numbers before; any edit to `forces.rs` could
//! have moved them and nobody would have seen it.
//!
//! **This is also the acceptance test for the per-track drive** (`docs/contact-and-tracks-program.md`,
//! Wave 4). That rewrite exists mostly to change how a climb FAILS — today a face past the ceiling
//! zeroes the forward speed and the hull simply stops, which is the least readable failure mode
//! available; with two track forces the uphill belt loses load, spins, and the hull slews off and
//! slides back down, which is a failure you can see and learn from. The failure mode is the point.
//! **The envelope below is not supposed to move with it.** A hull that can suddenly scrabble up
//! faces the map contract walls off has not been improved, it has been unmapped.

use game_core::VehicleKind;
use glam::Vec3;
use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, sample_tank_terrain_contact,
    step_custom_tank_controller_on_contact,
};
use terrain::HeightMap;

const DT: f32 = 1.0 / 60.0;
const CELLS: usize = 260;
const CELL_M: f32 = 3.0;
/// Where the flat apron ends and the face begins.
const FACE_Z: f32 = 240.0;
/// Height a hull must gain for the attempt to count as a climb rather than a scrabble.
const CLIMB_GAIN_M: f32 = 3.0;
/// Band the recorded envelope must hold inside. Tight: on one binary the search is bit-stable, so
/// this is slack for a different compiler, not licence to drift.
const TOLERANCE: f32 = 0.01;

/// Recorded 2026-08-06 on master. Steepest grade (rise/run) each hull will climb, by the angle it
/// takes the face at — head-on first, then 15°, 30°, 45°, 60° off square.
///
/// Read the two rows together, because the pair is the whole design:
///
/// * **from a standstill the vehicle matters** — a Tiger II starts a 26° face where a T-54 starts
///   a 29° one, which is power-to-weight showing through;
/// * **with a run-up every hull is identical** — 0.68 / 0.70 / 0.79 / 0.96 / 1.36, to the number,
///   because momentum makes them all grip-limited rather than power-limited. Climbing is a
///   DRIVER's skill and not a stat you can buy, and that is worth keeping deliberately.
///
/// And the headline the whole thing rests on: 34° head-on becomes **54° at sixty degrees off
/// square**. Twenty degrees of slope, bought with nothing but the angle of attack.
const ENVELOPE: &str = "\
# vehicle   approach  standing  run_up
T54_1951     0        0.56      0.68
T54_1951    15        0.56      0.70
T54_1951    30        0.61      0.79
T54_1951    45        0.74      0.96
T54_1951    60        0.73      1.36
TigerII      0        0.49      0.68
TigerII     15        0.49      0.70
TigerII     30        0.53      0.79
TigerII     45        0.64      0.96
TigerII     60        0.69      1.36
T34_85       0        0.56      0.68
T34_85      15        0.56      0.70
T34_85      30        0.62      0.79
T34_85      45        0.76      0.96
T34_85      60        0.73      1.36
";

#[test]
fn the_climbing_envelope_holds() {
    let mut failures = Vec::new();
    let mut rendered = String::from("# vehicle   approach  standing  run_up\n");
    for line in ENVELOPE.lines().filter(|line| !line.trim_start().starts_with('#')) {
        let mut fields = line.split_whitespace();
        let name = fields.next().expect("a vehicle");
        let approach: f32 = fields.next().expect("an angle").parse().expect("a number");
        let was_standing: f32 = fields.next().expect("standing").parse().expect("a number");
        let was_run_up: f32 = fields.next().expect("run-up").parse().expect("a number");
        let kind = vehicle(name);

        let standing = steepest(kind, approach.to_radians(), 2.0);
        let run_up = steepest(kind, approach.to_radians(), 120.0);
        rendered.push_str(&format!(
            "{name:<11} {approach:>2.0}        {standing:.2}      {run_up:.2}\n"
        ));
        for (label, was, is) in
            [("standing", was_standing, standing), ("run-up", was_run_up, run_up)]
        {
            if (is - was).abs() > TOLERANCE {
                failures.push(format!(
                    "{name} at {approach:.0}° {label}: was {was:.2}, is {is:.2} ({:.0}° vs {:.0}°)",
                    is.atan().to_degrees(),
                    was.atan().to_degrees()
                ));
            }
        }
    }

    println!("{rendered}");
    assert!(
        failures.is_empty(),
        "the climbing envelope moved. This is a contract, not a measurement: climbing is a \
         discipline by decision, and a hull that can suddenly scrabble up faces the map contract \
         walls off has not been improved, it has been unmapped. If the change was deliberate, \
         paste the table below over `ENVELOPE` and say WHY.\n\n{}\n\nconst ENVELOPE: &str = \"\\\n{}\";\n",
        failures.join("\n"),
        rendered
    );
}

/// The technique is the point: taking a face obliquely must genuinely buy slope, or "climbing as a
/// discipline" is a slogan rather than a mechanic.
#[test]
fn the_angle_of_attack_is_what_earns_the_climb() {
    let square = steepest(VehicleKind::T54_1951, 0.0, 120.0);
    let oblique = steepest(VehicleKind::T54_1951, 60.0_f32.to_radians(), 120.0);
    println!(
        "run-up: {:.0}° head-on, {:.0}° at sixty degrees off square",
        square.atan().to_degrees(),
        oblique.atan().to_degrees()
    );
    assert!(
        oblique.atan().to_degrees() - square.atan().to_degrees() >= 15.0,
        "the approach angle must buy real slope: {:.0}° against {:.0}°",
        oblique.atan().to_degrees(),
        square.atan().to_degrees()
    );
}

/// With momentum every hull climbs the same face. Climbing is a driver's skill, not a stat — a
/// heavy must not be able to buy its way up a bank a medium cannot take.
#[test]
fn a_committed_run_up_climbs_the_same_for_everybody() {
    let envelope: Vec<f32> = [VehicleKind::T54_1951, VehicleKind::TigerII, VehicleKind::T34_85]
        .into_iter()
        .map(|kind| steepest(kind, 45.0_f32.to_radians(), 120.0))
        .collect();
    let spread = envelope.iter().fold(0.0_f32, |worst, &grade| worst.max(grade))
        - envelope.iter().fold(f32::INFINITY, |best, &grade| best.min(grade));
    println!("run-up at 45°, across the fleet: {envelope:?} (spread {spread:.3})");
    assert!(spread <= TOLERANCE, "a committed climb must not depend on the vehicle: {envelope:?}");
}

fn vehicle(name: &str) -> VehicleKind {
    match name {
        "T54_1951" => VehicleKind::T54_1951,
        "TigerII" => VehicleKind::TigerII,
        "T34_85" => VehicleKind::T34_85,
        other => panic!("{other} is not in the climbing table"),
    }
}

/// Steepest grade this hull gains [`CLIMB_GAIN_M`] of height on, taking the face at `heading`.
fn steepest(kind: VehicleKind, heading: f32, run_up_m: f32) -> f32 {
    let (mut low, mut high) = (0.0_f32, 1.4_f32);
    for _ in 0..12 {
        let mid = 0.5 * (low + high);
        if climbs(kind, mid, heading, run_up_m) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn climbs(kind: VehicleKind, grade: f32, heading: f32, run_up_m: f32) -> bool {
    let spec = kind.spec();
    let settings = TankControllerSettings::from_spec(&spec);
    let map = face(grade);
    let mut state = TankKinematicState {
        position: Vec3::new(CELLS as f32 * CELL_M * 0.5, 0.0, FACE_Z - run_up_m.max(1.0)),
        yaw_rad: heading,
        ..Default::default()
    };
    let input = TankControlInput { throttle: 1.0, steer: 0.0, brake: 0.0 };
    for _ in 0..1_800 {
        let Some(contact) = sample_tank_terrain_contact(
            &map,
            state.position,
            state.yaw_rad,
            settings.ground_probe_length_m,
            &[],
            None,
        ) else {
            return true; // off the far end of the ramp: it climbed further than the map is long
        };
        state.position.y = contact.height_m;
        step_custom_tank_controller_on_contact(&mut state, input, &settings, contact, DT);
        if state.position.y > CLIMB_GAIN_M {
            return true;
        }
    }
    false
}

/// A flat apron that turns into a constant grade at [`FACE_Z`].
fn face(grade: f32) -> HeightMap {
    let mut samples = Vec::with_capacity(CELLS * CELLS);
    for z in 0..CELLS {
        for _ in 0..CELLS {
            let world_z = z as f32 * CELL_M;
            samples.push(if world_z < FACE_Z { 0.0 } else { (world_z - FACE_Z) * grade });
        }
    }
    HeightMap::new(CELLS, CELLS, CELL_M, samples).expect("the ramp's dimensions are fixed")
}
