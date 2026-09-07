use game_core::{ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn movement_blooms_dispersion_and_aim_time_recovers_it() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
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

/// S23 (GDD reconciliation row 32, on the record): the aim time is THREE e-folds — after one
/// `aim_time_seconds` of standing still 95 % of a bloom is gone (a T-54's "2.5 s" settles the
/// way WoT's 0.8 s does) — and the movement bloom is ADDITIVE, so a hull at full speed settles
/// where the bloom rate meets the recovery: base + bloom·(v/v_max)·aim_time/3 = 2.9 + 5.0·2.5/3
/// = 7.07 mrad for the T-54 (2.44× its base). Both numbers are the model's, locked here so a
/// re-tune shows as a number, not a feel.
#[test]
fn the_aim_time_is_three_e_folds_and_full_speed_settles_at_the_recorded_bloom() {
    let step = FixedTimestep::from_hz(60);
    let spec = TankSpec::t54_1951();
    let base = spec.gun.dispersion_mrad;

    // Standing still after one shot: the shot bloom decays to 5 % in one aim time.
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), spec.clone(), Vec3::ZERO);
    for _ in 0..300 {
        state.apply_commands(&[(tank, TankCommand::idle())], step);
    }
    assert!((state.tank(tank).expect("tank").aim_dispersion_mrad - base).abs() < 1.0e-3);
    state.apply_commands(&[(tank, fire_command())], step);
    let bloomed = state.tank(tank).expect("tank").aim_dispersion_mrad - base;
    assert!(bloomed > 3.0, "the shot bloom is on the sight: {bloomed}");
    let ticks = (spec.gun.aim_time_seconds * 60.0).round() as usize;
    for _ in 0..ticks {
        state.apply_commands(&[(tank, TankCommand::idle())], step);
    }
    let left = (state.tank(tank).expect("tank").aim_dispersion_mrad - base) / bloomed;
    assert!(
        (left - (-3.0_f32).exp()).abs() < 0.006,
        "after one aim time {:.1} % of the bloom is left (three e-folds: 5.0 %)",
        left * 100.0
    );

    // Full speed on the flat: the additive bloom settles against the recovery.
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), spec.clone(), Vec3::ZERO);
    for _ in 0..1500 {
        state.apply_commands(&[(tank, TankCommand::drive(1.0, 0.0))], step);
    }
    let driving = state.tank(tank).expect("tank");
    let speed_fraction = driving.velocity_mps.length() / spec.max_forward_speed_mps;
    assert!(speed_fraction > 0.95, "the T-54 reaches its top speed on the flat: {speed_fraction}");
    let expected =
        base + spec.gun.movement_bloom_mrad * speed_fraction * spec.gun.aim_time_seconds / 3.0;
    let settled = driving.aim_dispersion_mrad;
    assert!((settled - expected).abs() < 0.1, "settled {settled} vs the closed form {expected}");
    assert!((settled - 7.07).abs() < 0.3, "the recorded 7.07 mrad at full speed: {settled}");
    assert!((settled / base - 2.44).abs() < 0.1, "2.44× the base");
}

#[test]
fn firing_applies_shot_bloom_and_uses_deterministic_dispersion() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
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
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
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
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);
    let full_rate_step = TankSpec::t54_1951().turret_rotation_rad_s * step.dt_seconds();
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
