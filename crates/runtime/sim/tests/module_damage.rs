use std::f32::consts::PI;

use game_core::{DamageCause, ModuleSlot, TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

#[test]
fn penetrating_front_hit_damages_the_gun_module() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    // Depress onto the middle of the T-54's REAL glacis plate (the plane spans 1.0–1.58 m of
    // height): the low 1951 casting sits below the T-55A muzzle line, so a level shot clears
    // the hull. Dispersion is zeroed so the shot cannot wander onto the deck edge.
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = -0.010;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
    }
    let gun_hp_before = module_hp(&state, target, ModuleSlot::Gun);

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("penetrating hit event");
    assert!(event.penetrated);
    assert_eq!(event.module, Some(ModuleSlot::Gun));
    assert_eq!(event.cause, DamageCause::Shell);
    assert_eq!(module_hp(&state, target, ModuleSlot::Gun), 0);
    assert!(module_hp(&state, target, ModuleSlot::Gun) < gun_hp_before);
}

#[test]
fn turret_side_penetration_can_destroy_ammo_rack_module() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(-55.0, 0.0, 0.0));
    let target = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        // Low over the split: the shot must land on the *turret box side* (the visible turret,
        // |x| = turret_half_width), and the deterministic module roll there must clear the
        // ammo-rack chance.
        shooter.gun_pitch_rad = 0.002;
        shooter.spec.gun.shell.penetration_mm_at_100m = 240.0;
    }
    let ammo_before = module_hp(&state, target, ModuleSlot::AmmoRack);

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("ammo rack hit event");
    assert!(event.penetrated);
    assert_eq!(event.module, Some(ModuleSlot::AmmoRack));
    assert!(module_hp(&state, target, ModuleSlot::AmmoRack) < ammo_before);
}

#[test]
fn rear_side_penetration_hits_engine_volume_instead_of_generic_side_module() {
    let mut state = SimulationState::new();
    // Keep the projectile body's lower edge clear of the real track band: this test targets the
    // engine-bay side plate, while the separate track tests own grazing running-gear behavior.
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(-55.0, 0.0, -2.4));
    let target = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        shooter.gun_pitch_rad = -0.012;
        shooter.spec.gun.shell.penetration_mm_at_100m = 260.0;
    }
    let engine_before = module_hp(&state, target, ModuleSlot::Engine);
    let suspension_before = module_hp(&state, target, ModuleSlot::Suspension);

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("engine-bay hit event");
    assert!(event.penetrated);
    assert_eq!(event.armor_facing, game_core::ArmorFacing::HullSide);
    assert_eq!(event.armor_zone, game_core::ArmorZone::HullSide);
    assert_eq!(event.module, Some(ModuleSlot::Engine));
    assert!(module_hp(&state, target, ModuleSlot::Engine) < engine_before);
    assert_eq!(module_hp(&state, target, ModuleSlot::Suspension), suspension_before);
}

#[test]
fn high_explosive_near_miss_on_armor_can_throw_tracks_without_full_penetration() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::tiger_i_ausf_e(), Vec3::ZERO);
    let target =
        state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 55.0));
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.spec.gun.shell = game_core::ShellSpec::high_explosive(88.0, 600.0, 22.0, 300, 3.5);
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let target_hp = state.tank(target).expect("target").hit_points;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("HE surface hit event");
    assert!(!event.penetrated);
    assert_eq!(event.module, Some(ModuleSlot::Suspension));
    assert!(event.damage_hp > 0);
    assert!(state.tank(target).expect("target").hit_points < target_hp);
    assert_eq!(module_hp(&state, target, ModuleSlot::Suspension), 0);
}

#[test]
fn destroyed_gun_prevents_firing() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    state.tank_mut(shooter).expect("shooter").modules.damage(ModuleSlot::Gun, u32::MAX);

    state.apply_commands(&[(shooter, fire_command())], FixedTimestep::from_hz(60));

    let tank = state.tank(shooter).expect("shooter");
    assert!(state.shells().is_empty(), "destroyed gun must not spawn a shell");
    assert_eq!(tank.reload_remaining_s, 0.0, "failed shot must not start reload");
}

#[test]
fn knocked_out_tank_ignores_drive_aim_and_fire_commands() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(8.0, 0.0, 8.0));
    {
        let tank = state.tank_mut(tank).expect("tank");
        tank.hit_points = 0;
        tank.velocity_mps = Vec3::new(3.0, 0.0, 0.0);
    }
    let before = state.tank(tank).expect("tank").clone();

    state.apply_commands(
        &[(
            tank,
            TankCommand {
                throttle: 1.0,
                steer: 1.0,
                turret_yaw_delta: 1.0,
                gun_pitch_delta: 1.0,
                fire: true,
                ..TankCommand::idle()
            },
        )],
        FixedTimestep::from_hz(60),
    );

    let after = state.tank(tank).expect("tank");
    assert_eq!(after.position, before.position);
    assert_eq!(after.yaw_rad, before.yaw_rad);
    assert_eq!(after.turret_yaw_rad, before.turret_yaw_rad);
    assert_eq!(after.gun_pitch_rad, before.gun_pitch_rad);
    assert_eq!(after.reload_remaining_s, before.reload_remaining_s);
    assert_eq!(after.velocity_mps, Vec3::ZERO);
    assert!(state.shells().is_empty(), "knocked-out tank must not fire");
}

#[test]
fn fixed_casemate_ignores_turret_yaw_commands() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::jagdtiger(), Vec3::ZERO);
    state.tank_mut(tank).expect("tank").turret_yaw_rad = 0.35;

    state.apply_commands(
        &[(
            tank,
            TankCommand { turret_yaw_delta: 1.0, gun_pitch_delta: 1.0, ..TankCommand::idle() },
        )],
        FixedTimestep::from_hz(60),
    );

    let after = state.tank(tank).expect("tank");
    assert_eq!(after.turret_yaw_rad, 0.0);
    assert!(after.gun_pitch_rad > 0.0);
}

#[test]
fn spawned_tank_uses_module_health_from_assembled_loadout() {
    let mut loadout = VehicleKind::T55A.default_loadout();
    loadout.gun.hit_points = 1;
    let spec = loadout.assemble(VehicleKind::T55A);
    let mut state = SimulationState::new();

    let tank = state.spawn_tank(TeamId(1), spec, Vec3::ZERO);

    assert_eq!(module_hp(&state, tank, ModuleSlot::Gun), 1);
}

fn run_until_shell_resolved(state: &mut SimulationState, shooter: TankId) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..30 {
        state.apply_commands(&[], step);
        if !state.damage_events().is_empty() {
            return;
        }
    }
    panic!("shell should resolve against target");
}

fn module_hp(state: &SimulationState, tank: TankId, slot: ModuleSlot) -> u32 {
    state.tank(tank).expect("tank").modules.hit_points(slot)
}

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}
