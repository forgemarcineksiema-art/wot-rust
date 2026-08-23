use std::f32::consts::PI;

use game_core::{DamageCause, ModuleSlot, TankId, TankSpec, TeamId, VehicleKind};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

mod common;
use common::{fire_command, pitch_at_t54_bustle, run_until_shell_resolved};

#[test]
fn penetrating_centerline_hit_passes_between_racks_and_reaches_the_engine() {
    let mut state = SimulationState::new();
    // The D-10T2S: with the facet smear retired (#428) the stock D-10T's 185 mm meets an
    // honest 174 mm glacis and penetrates with single-digit residual — not enough to walk the
    // interior to the engine. This test is about the INTERIOR pass-through, so it fires the
    // round with the budget to make the walk.
    let mut loadout = game_core::VehicleKind::T54_1951.default_loadout();
    let d10t2s = game_core::VehicleKind::T54_1951
        .gun_options()
        .into_iter()
        .find(|gun| gun.spec.name == "100 mm D-10T2S")
        .expect("the T-54 offers the D-10T2S");
    loadout.try_install_gun(d10t2s).expect("the D-10T2S fits");
    let shooter =
        state.spawn_tank(TeamId(1), loadout.assemble(game_core::VehicleKind::T54_1951), Vec3::ZERO);
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
    let ammo_hp_before = module_hp(&state, target, ModuleSlot::AmmoRack);
    let engine_hp_before = module_hp(&state, target, ModuleSlot::Engine);
    let gun_hp_before = module_hp(&state, target, ModuleSlot::Gun);

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("penetrating hit event");
    assert!(event.penetrated);
    assert_eq!(event.module, Some(ModuleSlot::Engine));
    assert_eq!(event.cause, DamageCause::Shell);
    assert!(module_hp(&state, target, ModuleSlot::Engine) < engine_hp_before);
    assert_eq!(module_hp(&state, target, ModuleSlot::AmmoRack), ammo_hp_before);
    assert_eq!(module_hp(&state, target, ModuleSlot::Gun), gun_hp_before);
}

#[test]
fn turret_rear_penetration_can_destroy_ammo_rack_module() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, -55.0));
    // Five ready rounds live in the T-54's bustle. A rear-turret penetration meets them.
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::ZERO);
    {
        let pitch = pitch_at_t54_bustle(&state, shooter, target);
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        // Through the turret rear into the five ready clips (not the hull tub).
        shooter.gun_pitch_rad = pitch;
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
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(-55.0, 0.0, -2.4));
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::ZERO);
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
fn high_explosive_on_the_glacis_hurts_without_penetrating() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::tiger_i_ausf_e(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.spec.gun.shell = game_core::ShellSpec::high_explosive(88.0, 600.0, 22.0, 300, 3.5);
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.gun_pitch_rad = -0.010;
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let target_hp = state.tank(target).expect("target").hit_points;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("HE surface hit event");
    assert!(!event.penetrated);
    assert!(event.damage_hp > 0);
    assert!(state.tank(target).expect("target").hit_points < target_hp);
}

#[test]
fn destroyed_gun_prevents_firing() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    state.tank_mut(shooter).expect("shooter").modules.damage(ModuleSlot::Gun, u32::MAX);

    state.apply_commands(&[(shooter, fire_command())], FixedTimestep::from_hz(60));

    let tank = state.tank(shooter).expect("shooter");
    assert!(state.shells().is_empty(), "destroyed gun must not spawn a shell");
    assert_eq!(tank.reload_remaining_s, 0.0, "failed shot must not start reload");
}

#[test]
fn knocked_out_tank_ignores_drive_aim_and_fire_commands() {
    let mut state = SimulationState::new();
    let tank = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(8.0, 0.0, 8.0));
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
    let mut loadout = VehicleKind::T54_1951.default_loadout();
    loadout.gun.hit_points = 1;
    let spec = loadout.assemble(VehicleKind::T54_1951);
    let mut state = SimulationState::new();

    let tank = state.spawn_tank(TeamId(1), spec, Vec3::ZERO);

    assert_eq!(module_hp(&state, tank, ModuleSlot::Gun), 1);
}

fn module_hp(state: &SimulationState, tank: TankId, slot: ModuleSlot) -> u32 {
    state.tank(tank).expect("tank").modules.hit_points(slot)
}

/// Ignition is EARNED, and this is the rule that earns it.
///
/// A fire needs hot fragments in something flammable, so it asks for fire-level energy at the
/// component (`FIRE_ENERGY_MM`) — well above what merely gets inside. What that buys is the case
/// below: a round that gets in and FINISHES the engine without carrying enough left to light it.
///
/// Before this gate, any penetrating engine kill lit the deck and the fire meant nothing. Since
/// the frequency-relief pass a single round no longer destroys a HEALTHY engine at all (see
/// `a_single_penetration_wounds_a_healthy_module_it_does_not_destroy_it`), so this scenario
/// starts from an engine already fighting at 40 hp — the second-hit case. The two shots differ
/// only in muzzle penetration; both finish the engine, and only the energetic one burns.
///
/// Note what the tuned threshold implies, and why it is the point: a round needs ~360 mm of muzzle
/// penetration to light this engine THROUGH THE GLACIS, and the era's real guns carry 175-200. So a
/// frontal hit essentially never starts a fire — fires are what the flank and the rear cost you.
#[test]
fn wrecking_an_engine_is_not_the_same_as_lighting_it() {
    let spent = engine_kill_shot(260.0);
    let energetic = engine_kill_shot(380.0);

    assert_eq!(spent.engine_hp, 0, "the spent round still finishes the wounded engine");
    assert_eq!(energetic.engine_hp, 0, "so does the energetic one");
    assert!(
        !spent.engine_fire,
        "a round with nothing left to throw wrecks the engine WITHOUT lighting it"
    );
    assert!(energetic.engine_fire, "a round with fragments to spare lights the deck");
}

/// The frequency-relief promise itself (user verdict 2026-08-18: "obecnie nie da się grać"):
/// one penetration WOUNDS a healthy module, it does not destroy it. The same energetic
/// centreline shot that finishes a wounded engine above leaves a healthy one alive and
/// degraded — running at its damage floor, repairable, but running. `MODULE_WOUND_SCALE`
/// carries this promise; the battle-level reading is locked by
/// `battle_host/tests/battle_statistics.rs`.
#[test]
fn a_single_penetration_wounds_a_healthy_module_it_does_not_destroy_it() {
    let fresh = engine_shot_with(380.0, None);
    assert!(fresh.engine_hp > 0, "a healthy engine survives even an energetic single penetration");
    assert!(
        fresh.engine_hp < fresh.engine_full_hp,
        "but it does not shrug it off either — the hit is a real wound"
    );
    assert!(!fresh.engine_fire, "an engine that survived the hit is not burning");
}

struct EngineKill {
    engine_hp: u32,
    engine_full_hp: u32,
    engine_fire: bool,
}

/// One centreline glacis penetration into an engine already worn down to 40 hp — the
/// second-hit case a single round can actually finish since the frequency-relief pass.
fn engine_kill_shot(penetration_mm_at_100m: f32) -> EngineKill {
    engine_shot_with(penetration_mm_at_100m, Some(40))
}

/// One centreline glacis penetration into the engine bay at the given muzzle penetration,
/// against an engine pre-worn down to `engine_worn_to_hp` (or healthy for `None`).
fn engine_shot_with(penetration_mm_at_100m: f32, engine_worn_to_hp: Option<u32>) -> EngineKill {
    let mut state = SimulationState::new();
    // The D-10T2S: with the facet smear retired (#428) the stock D-10T's 185 mm meets an
    // honest 174 mm glacis and penetrates with single-digit residual — not enough to walk the
    // interior to the engine. This test is about the INTERIOR pass-through, so it fires the
    // round with the budget to make the walk.
    let mut loadout = game_core::VehicleKind::T54_1951.default_loadout();
    let d10t2s = game_core::VehicleKind::T54_1951
        .gun_options()
        .into_iter()
        .find(|gun| gun.spec.name == "100 mm D-10T2S")
        .expect("the T-54 offers the D-10T2S");
    loadout.try_install_gun(d10t2s).expect("the D-10T2S fits");
    let shooter =
        state.spawn_tank(TeamId(1), loadout.assemble(game_core::VehicleKind::T54_1951), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    let engine_full_hp;
    {
        let target = state.tank_mut(target).expect("target");
        target.yaw_rad = PI;
        engine_full_hp = target.modules.hit_points(ModuleSlot::Engine);
        if let Some(worn_to) = engine_worn_to_hp {
            target.modules.damage(ModuleSlot::Engine, engine_full_hp.saturating_sub(worn_to));
        }
    }
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = -0.010;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        shooter.spec.gun.shell.penetration_mm_at_100m = penetration_mm_at_100m;
    }
    run_until_shell_resolved(&mut state, shooter);
    let event = state.damage_events().last().expect("an impact");
    assert!(event.penetrated, "both shots must get inside at {penetration_mm_at_100m} mm");
    let tank = state.tank(target).expect("target");
    EngineKill {
        engine_hp: tank.modules.hit_points(ModuleSlot::Engine),
        engine_full_hp,
        engine_fire: tank.engine_fire,
    }
}
