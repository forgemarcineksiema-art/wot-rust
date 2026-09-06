//! Leaving a hull is ONE decision.
//!
//! A perforating round has two visible consequences on the far side: the wound it cuts in the
//! exit plate, and the fact that it is still a projectile out there. Those were decided from two
//! different budgets — the wound from what survived the internal path AND the exit plate, the
//! flight from the entry plate alone — so a round that had spent itself wrecking the engine
//! still sailed out of a plate the game had (correctly) left whole, and could go on to hit the
//! next tank.
//!
//! The invariant here is one-directional on purpose, because that is the honest one: whether a
//! kinetic round continues also depends on it being kinetic and on a minimum residual, but
//! NOTHING may be flying beyond a hull it never opened.

use std::f32::consts::PI;

use game_core::{BreachFace, TankId, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fire() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

/// Fire one shot from `gun_vehicle` at `target_vehicle` and report, once the shell has resolved:
/// whether a round is flying on past that target, and whether that target carries an exit wound.
fn shot(
    gun_vehicle: VehicleKind,
    target_vehicle: VehicleKind,
    gun_pitch_rad: f32,
    range_m: f32,
) -> (bool, bool) {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), gun_vehicle.spec(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), target_vehicle.spec(), Vec3::new(0.0, 0.0, range_m));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = gun_pitch_rad;
    let step = FixedTimestep::from_hz(60);

    state.apply_commands(&[(shooter, fire())], step);
    let mut struck = false;
    for _ in 0..240 {
        state.apply_commands(&[], step);
        if state.damage_events().iter().any(|event| event.target == target) {
            struck = true;
            break;
        }
        if state.shells().is_empty() {
            break;
        }
    }
    assert!(struck, "{gun_vehicle:?} -> {target_vehicle:?} at {range_m} m never connected");

    let flying_on = state.shells().iter().any(|shell| shell.last_penetrated_target == Some(target));
    let exit_wound = state
        .tank(target)
        .expect("target")
        .armor_breaches
        .breaches()
        .iter()
        .any(|breach| breach.face == BreachFace::Egress);
    (flying_on, exit_wound)
}

/// Across guns, hulls and aim points: nothing flies out of a hull that has no exit wound.
#[test]
fn no_round_flies_on_from_a_hull_it_never_opened() {
    // A spread that reaches both answers: thin hulls a round crosses outright, thick ones that
    // swallow it, and aim points on the glacis, the hull and the turret.
    let cases: [(VehicleKind, VehicleKind, f32, f32); 8] = [
        (VehicleKind::T54_1951, VehicleKind::T34_85, -0.010, 35.0),
        (VehicleKind::T54_1951, VehicleKind::T34_85, 0.0, 35.0),
        (VehicleKind::T54_1951, VehicleKind::T54_1951, -0.007, 55.0),
        (VehicleKind::T54_1951, VehicleKind::TigerI, -0.008, 50.0),
        (VehicleKind::T54_1951, VehicleKind::TigerII, -0.008, 50.0),
        (VehicleKind::T34_85, VehicleKind::TigerII, -0.006, 40.0),
        (VehicleKind::TigerII, VehicleKind::T34_85, -0.012, 40.0),
        (VehicleKind::Centurion, VehicleKind::T34_85, -0.010, 40.0),
    ];

    let mut ever_flew = false;
    let mut ever_stopped = false;
    for (gun, target, pitch, range) in cases {
        let (flying_on, exit_wound) = shot(gun, target, pitch, range);
        assert!(
            !flying_on || exit_wound,
            "{gun:?} -> {target:?} at {range} m: a round is flying beyond a hull with no exit \
             wound. The exit plate and the continued flight must come from ONE budget."
        );
        ever_flew |= flying_on;
        ever_stopped |= !flying_on;
    }
    // The invariant is worthless if the fixture never produces both outcomes.
    assert!(ever_flew, "the spread must contain at least one genuine over-penetration");
    assert!(ever_stopped, "and at least one round the hull swallowed");
}

/// The exit plate is not free: crossing a hull costs its FAR side too, so the residual a round
/// carries out is strictly less than what it had after the plate it came in through.
#[test]
fn the_exit_plate_costs_the_round_that_leaves_through_it() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), VehicleKind::T54_1951.spec(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), VehicleKind::T34_85.spec(), Vec3::new(0.0, 0.0, 35.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.010;
    let step = FixedTimestep::from_hz(60);

    let stock_pen = VehicleKind::T54_1951.spec().gun.shell.penetration_mm_at_100m;
    state.apply_commands(&[(shooter, fire())], step);
    let mut entry_residual = None;
    for _ in 0..240 {
        state.apply_commands(&[], step);
        if let Some(event) = state.damage_events().iter().find(|event| event.target == target) {
            assert!(event.penetrated, "the fixture shot must perforate the T-34-85's glacis");
            entry_residual = Some(event.shell_penetration_mm - event.effective_armor_mm);
            break;
        }
    }
    let entry_residual = entry_residual.expect("the shot connects");
    let carried_on = state
        .shells()
        .iter()
        .find(|shell| shell.last_penetrated_target == Some(TankId(target.0)))
        .map(|shell| shell.shell.penetration_mm_at_100m);

    if let Some(carried_on) = carried_on {
        assert!(
            carried_on < entry_residual,
            "the far plate must take its share: carried {carried_on:.1} mm out of {entry_residual:.1} mm \
             left after the entry plate"
        );
        assert!(carried_on < stock_pen, "and the round is blunted, never refreshed");
    }
}
