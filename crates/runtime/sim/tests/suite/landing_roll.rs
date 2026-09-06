//! THE ONE WAY A TANK CAN STILL GO OVER — P3.1 of `docs/contact-and-tracks-program.md`.
//!
//! The rollover arithmetic in that program checked every path to putting a tank on its roof and
//! found exactly one within reach. Not a turn (a hull breaks traction at 0.99 g and would need
//! 1.14–1.36 to lean), not a side slope (52° against a map contract that stops at 34°), not a curb
//! (8.3 m/s of pure lateral velocity, instantly arrested), not a broadside ram (friction supplies
//! 8% of the impulse needed). One: an **asymmetric landing**, where a hull that took off level
//! comes down across a bank and one track touches first.
//!
//! The decision was to leave that path reachable and make it recoverable — the hull lurches most
//! of the way to its tipping angle, the suspension pays, and it comes back down. A tank is never
//! lost to terrain.
//!
//! Nothing new is stored to do it. The excursion is added to the authoritative roll, and the
//! attitude system's existing rate limit walks it back. A spring would have carried oscillation
//! state into the authoritative simulation, and every attitude in this game is deliberately
//! sprung (Inny Poziom G7): the excursion lands on the authoritative attitude spring, whose state
//! is on the wire, so replays and the client predictor stay exact through it.

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

const CELLS: usize = 120;
const CELL_M: f32 = 2.0;
/// The plateau's lip, in world metres along +z.
const LIP_Z: f32 = 120.0;
/// How far the hull falls off it.
const DROP_M: f32 = 3.0;

/// A plateau that ends in a drop onto ground rolled `bank_rad` about the direction of travel.
fn plateau_over_bank(bank_rad: f32) -> HeightMap {
    let mut samples = Vec::with_capacity(CELLS * CELLS);
    for z in 0..CELLS {
        for x in 0..CELLS {
            let (world_x, world_z) = (x as f32 * CELL_M, z as f32 * CELL_M);
            samples.push(if world_z < LIP_Z {
                DROP_M
            } else {
                (world_x - CELLS as f32 * CELL_M * 0.5) * bank_rad.tan()
            });
        }
    }
    HeightMap::new(CELLS, CELLS, CELL_M, samples).expect("the test map's dimensions are fixed")
}

/// Drive off the lip onto ground banked by `bank_rad` and report the worst roll the hull reached
/// and the roll it had settled to a second later.
fn drive_off_the_lip(bank_rad: f32) -> (f32, f32, f32) {
    let map = plateau_over_bank(bank_rad);
    let mut state = SimulationState::new();
    let tank = state.spawn_tank_with_yaw(
        TeamId(1),
        TankSpec::t54_1951(),
        Vec3::new(CELLS as f32 * CELL_M * 0.5, DROP_M, LIP_Z - 60.0),
        0.0,
    );
    let go = [(tank, TankCommand::drive(1.0, 0.0))];

    let mut worst_roll = 0.0_f32;
    let mut landed_at = None;
    for tick in 0..900 {
        state.apply_commands_on_terrain(&go, step(), &map);
        let roll = state.tank(tank).expect("tank").hull_roll_rad;
        if state.tank(tank).expect("tank").position.z > LIP_Z && landed_at.is_none() {
            landed_at = Some(tick);
        }
        if landed_at.is_some_and(|first| tick > first) {
            worst_roll = worst_roll.max(roll.abs());
        }
    }
    let settled = state.tank(tank).expect("tank").hull_roll_rad;
    (worst_roll, settled.abs(), bank_rad)
}

/// A hull that comes down across a bank goes over far enough to frighten the crew, and comes back.
#[test]
fn an_asymmetric_landing_lurches_the_hull_and_it_recovers() {
    let (worst, settled, bank) = drive_off_the_lip(10.0_f32.to_radians());
    println!(
        "3 m drop onto a {:.0}° bank: peak roll {:.1}°, settled {:.1}°",
        bank.to_degrees(),
        worst.to_degrees(),
        settled.to_degrees()
    );
    assert!(
        worst.to_degrees() >= 25.0,
        "a one-track landing must genuinely lurch the hull, got {:.1}°",
        worst.to_degrees()
    );
    // ...and it settles back onto the plane it landed on, rather than staying over.
    assert!(
        (settled - bank).abs().to_degrees() <= 2.0,
        "the hull must come back down to the bank it is standing on: settled {:.1}° on a \
         {:.1}° bank",
        settled.to_degrees(),
        bank.to_degrees()
    );
}

/// The same fall onto level ground is a landing, not a lurch. Flat arrivals have no track to turn
/// about, and inventing a roll for them would make every hop theatrical.
#[test]
fn a_flat_landing_does_not_lurch_at_all() {
    let (worst, settled, _) = drive_off_the_lip(0.0);
    println!("3 m drop onto level ground: peak roll {:.2}°", worst.to_degrees());
    assert!(
        worst.to_degrees() <= 1.0,
        "a square landing must not roll the hull, got {:.2}°",
        worst.to_degrees()
    );
    assert!(settled.to_degrees() <= 1.0);
}

/// However hard the fall, the hull never passes its own tipping angle: the excursion is capped at
/// a fraction of it, which is the whole of "reachable but recoverable".
#[test]
fn no_landing_ever_puts_a_hull_past_its_tipping_angle() {
    let stability =
        game_core::stock_stability(game_core::VehicleKind::T54_1951).expect("blueprint");
    let tipping = (stability.tip_edge_m / stability.com_height_m).atan();
    for bank_deg in [8.0_f32, 15.0, 25.0, 30.0] {
        let (worst, _, _) = drive_off_the_lip(bank_deg.to_radians());
        println!(
            "{bank_deg:.0}° bank: peak roll {:.1}° against a {:.1}° tipping angle",
            worst.to_degrees(),
            tipping.to_degrees()
        );
        assert!(
            worst < tipping,
            "a landing on a {bank_deg:.0}° bank rolled the hull {:.1}°, past its {:.1}° tipping \
             angle — the tank was lost to terrain",
            worst.to_degrees(),
            tipping.to_degrees()
        );
    }
}
