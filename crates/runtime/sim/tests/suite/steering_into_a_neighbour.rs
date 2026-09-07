//! DRIVING ALONGSIDE AND STEERING IN — the case the player reported, and the case every earlier
//! probe in this program was blind to.
//!
//! Reported: driving next to another tank, a shade slower, then steering into it — and the hull
//! goes in. Literally. Every measurement taken during Wave 1 said the overlap was 0.0000 m, and
//! they were all wrong the same way: they measured the distance between hull CENTRES along one
//! world axis. That number cannot see a corner swung into a flank. It reports two hulls as clear
//! while one of them is buried in the other.
//!
//! Everything here asks `physics::footprint_penetration_m` instead — the solver's own separating
//! axis test, so the answer to "are these inside each other" is the one the solver would give.

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use physics::{TankFootprint, TankObstacle, footprint_penetration_m};
use sim::{FixedTimestep, SimulationState, TankCommand};

fn step() -> FixedTimestep {
    FixedTimestep::from_hz(60)
}

/// The deepest either hull is inside the other, as the solver sees it.
fn penetration(state: &SimulationState, a: TankId, b: TankId) -> f32 {
    let hull = |id: TankId| {
        let tank = state.tank(id).expect("tank");
        TankObstacle::new(
            tank.position,
            tank.yaw_rad,
            TankFootprint::from_plan(tank.spec.hull_plan()),
        )
    };
    footprint_penetration_m(&hull(a), &hull(b))
}

/// **The promise, kept (the one program's X8).** Register H2 retired.
///
/// A hull may lean on its neighbour and never go inside it. It did: a contact was ONE point — the
/// midpoint of the two centres — and a hull pinned at one point is free to turn about it, so a
/// corner elsewhere buried itself 0.166 m steering in and 0.430 m leaning on a parked hull. The
/// contact now acts WHERE the plates meet: at the incident corner, and at the corner next to it
/// when a face lies flat on a face (`FootprintContact::points`), with each body's correction
/// shared across the constraints holding it — the Jacobi split that let the two-point manifold
/// ship without sinking a queue (the first manifold, unsplit, took a queue of three from no
/// motion to 0.11 m of sink).
///
/// What the solver is designed to leave alone is `POSITION_SLOP_M` — two centimetres — and a hull
/// pressed with full throttle AND full steer settles past it where the drive's push balances the
/// recovery velocity: measured 0.060–0.065 m on the three manoeuvres below with the manifold (the
/// same at four and eight solver passes, so it is the equilibrium of push against recovery, not
/// a solve that ran out of reach). The bound is that equilibrium and a hair, not the hole the bug
/// had — and it is not to be raised.
const TOLERATED_M: f32 = 0.07;

/// The reported case, exactly: side by side, the player a shade slower, steering into the
/// neighbour and holding it there.
#[test]
fn steering_into_a_neighbour_never_gets_inside_it() {
    let spec = TankSpec::t54_1951();
    let half_width = spec.hull_plan().half_width_m;
    let mut state = SimulationState::new();
    let player = state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let neighbour = state.spawn_tank_with_yaw(
        TeamId(2),
        spec.clone(),
        Vec3::new(2.0 * half_width + 0.4, 0.0, 0.0),
        0.0,
    );

    // The neighbour rolls gently ahead; the player follows a shade slower and leans left into it.
    let go = [(player, TankCommand::drive(0.55, 0.6)), (neighbour, TankCommand::drive(0.7, 0.0))];
    let mut worst = 0.0_f32;
    let mut worst_tick = 0;
    for tick in 0..600 {
        state.apply_commands(&go, step());
        let depth = penetration(&state, player, neighbour);
        if depth > worst {
            worst = depth;
            worst_tick = tick;
        }
    }
    println!("steering in: deepest penetration {worst:.4} m at tick {worst_tick}");
    assert!(
        worst <= TOLERATED_M,
        "steering into a neighbour buries the hull {worst:.3} m, past the {TOLERATED_M} m a lean is \
         allowed (H2 was 0.166 m; do not raise this)"
    );
}

/// The same lean against a neighbour that is standing still, which is how the report described the
/// worst of it.
#[test]
fn leaning_on_a_parked_neighbour_never_gets_inside_it() {
    let spec = TankSpec::t54_1951();
    let half_width = spec.hull_plan().half_width_m;
    let mut state = SimulationState::new();
    let player = state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let parked = state.spawn_tank_with_yaw(
        TeamId(2),
        spec.clone(),
        Vec3::new(2.0 * half_width + 0.1, 0.0, 0.0),
        0.0,
    );

    let go = [(player, TankCommand::drive(1.0, 1.0)), (parked, TankCommand::drive(0.0, 0.0))];
    let mut worst = 0.0_f32;
    for _ in 0..600 {
        state.apply_commands(&go, step());
        worst = worst.max(penetration(&state, player, parked));
    }
    println!("leaning on a parked hull: deepest penetration {worst:.4} m");
    assert!(
        worst <= TOLERATED_M,
        "leaning on a parked hull buries it {worst:.3} m, past the {TOLERATED_M} m a lean is allowed \
         (H2 was 0.44 m; do not raise this)"
    );
}

/// A pure pivot against a touching neighbour — the manoeuvre P1.4 measured as "zero overlap" with
/// an axis distance that could not see it.
#[test]
fn pivoting_against_a_neighbour_never_gets_inside_it() {
    let spec = TankSpec::t54_1951();
    let half_width = spec.hull_plan().half_width_m;
    let mut state = SimulationState::new();
    let player = state.spawn_tank_with_yaw(TeamId(1), spec.clone(), Vec3::ZERO, 0.0);
    let parked = state.spawn_tank_with_yaw(
        TeamId(2),
        spec.clone(),
        Vec3::new(2.0 * half_width + 0.02, 0.0, 0.0),
        0.0,
    );

    let go = [(player, TankCommand::drive(0.0, 1.0)), (parked, TankCommand::drive(0.0, 0.0))];
    let mut worst = 0.0_f32;
    for _ in 0..600 {
        state.apply_commands(&go, step());
        worst = worst.max(penetration(&state, player, parked));
    }
    println!("pivoting against a hull: deepest penetration {worst:.4} m");
    assert!(
        worst <= TOLERATED_M,
        "a pivot bores {worst:.3} m in, past the {TOLERATED_M} m allowed (H2)"
    );
}
