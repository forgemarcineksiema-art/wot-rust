use game_core::{ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn movement_blooms_dispersion_and_aim_time_recovers_it() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);
    let base = state.tank(tank).expect("tank").aim_dispersion_mrad;

    for _ in 0..30 {
        state.apply_commands(&[(tank, TankCommand::drive(1.0, 0.7))], step);
    }
    let bloomed = state.tank(tank).expect("tank").aim_dispersion_mrad;
    assert!(bloomed > base + 0.5, "moving tank should bloom aim circle");

    for _ in 0..240 {
        state.apply_commands(&[(tank, TankCommand::idle())], step);
    }
    let recovered = state.tank(tank).expect("tank").aim_dispersion_mrad;
    assert!(recovered < bloomed, "aim time should recover movement bloom");
    assert!(recovered >= base, "dispersion must not recover below gun base");
}

#[test]
fn firing_applies_shot_bloom_and_uses_deterministic_dispersion() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);
    for _ in 0..20 {
        state.apply_commands(&[(tank, TankCommand::drive(1.0, 0.0))], step);
    }
    let before_shot = state.tank(tank).expect("tank").aim_dispersion_mrad;

    state.apply_commands(&[(tank, fire_command())], step);

    let after_shot = state.tank(tank).expect("tank").aim_dispersion_mrad;
    let shell = state.shells().first().expect("shell");
    let velocity = shell.velocity_mps.normalize();
    assert!(after_shot > before_shot, "shot should add bloom");
    assert!(velocity.x.abs() > 1.0e-5, "dispersion should perturb shot yaw deterministically");
}

#[test]
fn damaged_gun_raises_minimum_dispersion_but_destroyed_gun_still_cannot_fire() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let full_min = state.tank(tank).expect("tank").aim_dispersion_mrad;
    {
        let tank = state.tank_mut(tank).expect("tank");
        tank.modules
            .damage(ModuleSlot::Gun, tank.spec.module_health.hit_points(ModuleSlot::Gun) / 2);
    }
    state.apply_commands(&[(tank, TankCommand::idle())], FixedTimestep::from_hz(60));
    let damaged_min = state.tank(tank).expect("tank").aim_dispersion_mrad;

    assert!(damaged_min > full_min, "partial gun damage should worsen aim");

    state.tank_mut(tank).expect("tank").modules.damage(ModuleSlot::Gun, u32::MAX);
    state.apply_commands(&[(tank, fire_command())], FixedTimestep::from_hz(60));
    assert!(state.shells().is_empty(), "destroyed gun must not fire");
}

#[test]
fn turret_traverse_uses_spec_rate_without_acceleration_ramp() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);
    let full_rate_step = TankSpec::t55a().turret_rotation_rad_s * step.dt_seconds();
    let command = TankCommand { turret_yaw_delta: 1.0, ..TankCommand::idle() };

    state.apply_commands(&[(tank, command)], step);
    let first_yaw = state.tank(tank).expect("tank").turret_yaw_rad;

    assert!(
        (first_yaw - full_rate_step).abs() < 1.0e-5,
        "turret should use the spec traverse rate immediately, got {first_yaw} vs {full_rate_step}"
    );

    let before_later_step = state.tank(tank).expect("tank").turret_yaw_rad;
    state.apply_commands(&[(tank, command)], step);
    let later_delta = state.tank(tank).expect("tank").turret_yaw_rad - before_later_step;

    assert!(
        (later_delta - full_rate_step).abs() < 1.0e-5,
        "turret traverse should stay at the spec rotation rate"
    );
}

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}
