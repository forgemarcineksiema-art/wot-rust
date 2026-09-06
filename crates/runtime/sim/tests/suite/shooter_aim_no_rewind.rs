//! The invariant that makes shooter-turret lag-compensation (audit N3) a NO-OP, locked.
//!
//! A rewind exists to reconstruct "where the shooter was aiming when they clicked". Here there
//! is nothing to reconstruct: the server folds the client's OWN ordered turret-delta stream
//! through the same integrator the client predicts with, and the shell leaves along that folded
//! aim. The value a rewind would recover is the value already in hand. This test pins that so a
//! future change to input timing cannot silently introduce a shooter-aim desync — which is the
//! only thing that could ever resurrect the demand for the (doctrine-forbidden) world rewind.
//!
//! See `docs/w1-networking-primer.md` §6 and `docs/server-first-policy.md` for the verdict.

use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

use game_core::{TankSpec, TeamId};

#[test]
fn the_shell_leaves_along_the_shooters_own_accumulated_aim() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    // Dispersion off: the barrel CENTRE is the shell's line, so the assertion reads the aim
    // itself, not a cone sample.
    {
        let tank = state.tank_mut(shooter).expect("shooter");
        tank.aim_dispersion_mrad = 0.0;
        tank.spec.gun.dispersion_mrad = 0.0;
    }

    // Traverse the turret with a stream of deltas — exactly the shape the client sends and
    // predicts locally. No absolute angle is ever transmitted; the turret is a fold of these.
    let slew = TankCommand { turret_yaw_delta: 0.7, ..TankCommand::idle() };
    for _ in 0..30 {
        state.apply_commands(&[(shooter, slew)], step);
    }
    let aim_yaw = {
        let tank = state.tank(shooter).expect("shooter");
        // World aim = hull yaw + turret yaw. The hull never turned, so this is the turret fold.
        tank.yaw_rad + tank.turret_yaw_rad
    };
    assert!(aim_yaw.abs() > 0.05, "the traverse must actually move the turret off centre");

    // Fire. The shell spawns from the LIVE turret this same tick (settle before fire), so it
    // leaves along `aim_yaw` — the value a rewind would have reconstructed.
    state.apply_commands(&[(shooter, TankCommand { fire: true, ..TankCommand::idle() })], step);
    let shell = state.shells().first().expect("the fire command spawned a shell");
    let heading = shell.velocity_mps.x.atan2(shell.velocity_mps.z);

    let delta = game_core::math::wrap_angle(heading - aim_yaw).abs();
    assert!(
        delta < 1.0e-3,
        "the shell leaves along the shooter's own folded aim ({aim_yaw} rad); heading {heading} \
         differs by {delta} rad — a rewind would only reconstruct the aim already used"
    );
}
