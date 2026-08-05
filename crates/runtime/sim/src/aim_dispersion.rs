use game_core::math::{gun_direction, splitmix64};
use game_core::{ModuleSlot, TankId, TankSpec};
use glam::Vec3;

use crate::{TankCommand, TankState};

/// Settled-minimum dispersion: the gun's base bloom, raised by partial gun-module damage.
/// `gun_damage_fraction` is `0` healthy .. `1` destroyed.
///
/// Public because the sight has to ask the same question the recovery curve answers: a wounded
/// gun settles onto a WIDER minimum, and "the aim has been taken" means reaching that minimum,
/// not the pristine one off the spec sheet.
pub fn base_dispersion_mrad(spec: &TankSpec, gun_damage_fraction: f32) -> f32 {
    spec.gun.dispersion_mrad * (1.0 + gun_damage_fraction.clamp(0.0, 1.0) * 1.5)
}

/// Aim-time recovery of the dispersion toward its settled minimum. Operates on a bare dispersion
/// value so the server (`TankState`) and the client predictor share one recovery curve.
pub fn recover_dispersion(
    aim_dispersion_mrad: &mut f32,
    spec: &TankSpec,
    gun_damage_fraction: f32,
    dt: f32,
) {
    let minimum = base_dispersion_mrad(spec, gun_damage_fraction);
    let aim_time = spec.gun.aim_time_seconds.max(0.05);
    let recovery = (-3.0 * dt / aim_time).exp();
    *aim_dispersion_mrad = minimum + (aim_dispersion_mrad.max(minimum) - minimum) * recovery;
}

/// Bloom added this tick from hull motion, steering, and turret/gun traverse.
pub(crate) fn command_bloom(
    aim_dispersion_mrad: &mut f32,
    spec: &TankSpec,
    gun_damage_fraction: f32,
    speed_mps: f32,
    command: TankCommand,
    dt: f32,
) {
    let speed_frac = (speed_mps / spec.max_forward_speed_mps.max(0.1)).clamp(0.0, 1.5);
    let hull_input = command.steer.abs() * 0.35;
    let aim_input = (command.turret_yaw_delta.abs() + command.gun_pitch_delta.abs()) * 0.25;
    let bloom = spec.gun.movement_bloom_mrad * (speed_frac + hull_input + aim_input) * dt;
    add_bloom(aim_dispersion_mrad, spec, gun_damage_fraction, bloom);
}

/// Gun-module damage as a `0..1` fraction, the input the neutral dispersion math expects.
pub(crate) fn gun_damage_fraction(tank: &TankState) -> f32 {
    let full_hp = tank.spec.module_health.hit_points(ModuleSlot::Gun).max(1) as f32;
    let live_hp = tank.modules.hit_points(ModuleSlot::Gun) as f32;
    (1.0 - live_hp / full_hp).clamp(0.0, 1.0)
}

pub(crate) fn recover_aim_dispersion(tank: &mut TankState, dt: f32) {
    let fraction = gun_damage_fraction(tank);
    recover_dispersion(&mut tank.aim_dispersion_mrad, &tank.spec, fraction, dt);
}

pub(crate) fn apply_shot_bloom(tank: &mut TankState) {
    let fraction = gun_damage_fraction(tank);
    let bloom = tank.spec.gun.shot_bloom_mrad;
    add_bloom(&mut tank.aim_dispersion_mrad, &tank.spec, fraction, bloom);
}

pub(crate) fn dispersed_gun_direction(tank: &TankState, tick: u64) -> Vec3 {
    let radius_rad = tank.aim_dispersion_mrad.max(0.0) * 0.001;
    let (unit_a, unit_b) = deterministic_pair(tank.id, tick, tank.dispersion_shot_index);
    let angle = unit_a * std::f32::consts::TAU;
    let radius = radius_rad * unit_b * unit_b;
    // Perturb in the hull frame, then carry the direction through the hull basis: the barrel —
    // and therefore its dispersion cone — tilts with the hull (on a level hull this degenerates
    // to the old planar formula exactly).
    (tank.hull_pose().basis()
        * gun_direction(
            tank.turret_yaw_rad + angle.cos() * radius,
            tank.gun_pitch_rad + angle.sin() * radius,
        ))
    .normalize()
}

fn add_bloom(
    aim_dispersion_mrad: &mut f32,
    spec: &TankSpec,
    gun_damage_fraction: f32,
    bloom_mrad: f32,
) {
    let maximum = spec.gun.max_dispersion_mrad.max(base_dispersion_mrad(spec, gun_damage_fraction));
    *aim_dispersion_mrad = (*aim_dispersion_mrad + bloom_mrad.max(0.0)).clamp(0.0, maximum);
}

fn deterministic_pair(tank_id: TankId, tick: u64, shot_index: u32) -> (f32, f32) {
    let mut value = tick ^ tank_id.0.rotate_left(17) ^ (shot_index as u64).rotate_left(31);
    value = splitmix64(value);
    let a = unit_float(value);
    value = splitmix64(value);
    let b = unit_float(value).max(0.15);
    (a, b)
}

fn unit_float(value: u64) -> f32 {
    ((value >> 40) as f32) / ((1u64 << 24) as f32)
}
