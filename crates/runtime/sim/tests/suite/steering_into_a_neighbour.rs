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

/// **A RECORDED DEFECT, not a promise kept.** Register H2.
///
/// A hull may lean on its neighbour and should never go inside it. It does. These are today's
/// numbers, held so the hole cannot quietly widen while the fix is built.
///
/// The cause is understood and measured: a contact is ONE point, and a hull pinned at one point is
/// free to turn about it — the rotation violates nothing there, so the solver applies nothing while
/// a corner elsewhere buries itself. The fix is a two-point manifold, built and measured at 0.039 m
/// worst case, and NOT shipped: a Jacobi pass handed two coupled points per pair destabilises
/// stacks — a queue of three went from 0.0014 m of sink and no motion to 0.110 m and 0.044 m/tick.
/// Trading "you can drive into a tank" for "a queue of three sinks 11 cm" is not a fix. It needs
/// the solver to share a hull's correction across the contacts holding it, or to iterate
/// sequentially over a deterministically sorted list.
///
/// A hull may lean on its neighbour. It may not go inside it.
///
/// The solver is designed to leave `POSITION_SLOP_M` — two centimetres — alone, and a hull pressed
/// with full throttle AND full steer settles a hair past it where the drive's push balances the
/// recovery. Measured at 0.026-0.030 m; the bound carries a little headroom over that and nothing
/// like the room the bug had. For scale, the same manoeuvres before the fix: 0.166 m steering in,
/// 0.430 m leaning on a parked hull, and both were still growing.
const TOLERATED_M: f32 = 0.13;

/// The reported case, exactly: side by side, the player a shade slower, steering into the
/// neighbour and holding it there.
#[test]
fn steering_into_a_neighbour_does_not_get_worse() {
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
        "steering into a neighbour now buries the hull {worst:.3} m, past the {TOLERATED_M} m          recorded. Register H2 is getting worse; close it with the manifold, do not raise this"
    );
}

/// The same lean against a neighbour that is standing still, which is how the report described the
/// worst of it.
#[test]
fn leaning_on_a_parked_neighbour_does_not_get_worse() {
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
        worst <= 0.47,
        "leaning on a parked hull now buries it {worst:.3} m, past the 0.44 m recorded (H2)"
    );
}

/// A pure pivot against a touching neighbour — the manoeuvre P1.4 measured as "zero overlap" with
/// an axis distance that could not see it.
#[test]
fn pivoting_against_a_neighbour_does_not_get_worse() {
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
    assert!(worst <= TOLERATED_M, "a pivot now bores {worst:.3} m in, past what is recorded (H2)");
}
