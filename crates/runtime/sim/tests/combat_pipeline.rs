use std::f32::consts::PI;

use game_core::{ArmorFacing, ArmorZone, ModuleSlot, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

#[test]
fn fire_command_spawns_shell_and_starts_reload() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let step = FixedTimestep::from_hz(60);

    state.apply_commands(&[(shooter, fire_command())], step);
    state.apply_commands(&[(shooter, fire_command())], step);

    let tank = state.tank(shooter).expect("shooter");
    assert_eq!(state.shells().len(), 1);
    assert!(state.shells()[0].position.z > tank.position.z);
    assert!(tank.reload_remaining_s > 0.0);
}

#[test]
fn shell_hit_penetrates_armor_and_applies_damage() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 55.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    // Depress the gun slightly so the round strikes the T-54's hull front: its low 1951 casting sits
    // below the T-55A muzzle line, so a perfectly level shot would clear the hull and hit the thick
    // mantlet. This keeps the test exercising a frontal hull penetration.
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.007;
    let target_hp = state.tank(target).expect("target").hit_points;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("penetrating hit event");
    assert_eq!(event.source, shooter);
    assert_eq!(event.target, target);
    assert!(event.penetrated);
    assert!(event.damage_hp > 0);
    assert!(state.tank(target).expect("target").hit_points < target_hp);
    assert!(state.shells().is_empty());
}

#[test]
fn shell_hit_that_fails_penetration_reports_bounce_without_damage() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::panther_ii(), Vec3::ZERO);
    let target =
        state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 55.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let target_hp = state.tank(target).expect("target").hit_points;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("bounce hit event");
    assert_eq!(event.target, target);
    assert!(!event.penetrated);
    assert_eq!(event.damage_hp, 0);
    assert_eq!(state.tank(target).expect("target").hit_points, target_hp);
}

#[test]
fn terrain_crossing_blocks_shell_before_target_hit() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(20.0, 0.0, 0.0));
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(20.0, 0.0, 18.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let terrain = ridge_heightmap();
    let target_hp = state.tank(target).expect("target").hit_points;
    let step = FixedTimestep::from_hz(60);

    state.apply_commands_on_terrain(&[(shooter, fire_command())], step, &terrain);

    for _ in 0..4 {
        state.apply_commands_on_terrain(&[], step, &terrain);
    }

    assert!(state.damage_events().is_empty(), "ridge should absorb the shell before target hit");
    assert_eq!(state.tank(target).expect("target").hit_points, target_hp);
    assert!(state.shells().is_empty(), "blocked shell should be removed at terrain impact");
}

#[test]
fn vehicle_specific_hitbox_width_is_used_for_shell_hits() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(1.8, 0.0, 0.0));
    let target =
        state.spawn_tank(TeamId(2), TankSpec::tiger_ii_ausf_b(), Vec3::new(0.0, 0.0, 55.0));
    {
        // At x = 1.8 only the Tiger II's full 3.75 m running gear is that wide — its 25° leaned
        // upper sides have honestly receded at muzzle height — so drop the shot into the track
        // band (~0.95 m) where the wide body genuinely lives. A narrower vehicle whiffs here.
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = -0.016;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("wide Tiger II hitbox should be hit");
    assert_eq!(event.target, target);
}

#[test]
fn high_side_hit_uses_turret_side_armor_not_hull_side_armor() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(-55.0, 0.0, 0.0));
    let target = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        // Level fire arrives mid-dome (~1.76 m): the T-55A's real cast turret, not a tall
        // phantom box — the old +0.006 sailed over the 2.03 m casting.
        shooter.gun_pitch_rad = 0.0;
        shooter.spec.gun.shell.penetration_mm_at_100m = 85.0;
    }
    let target_hp = state.tank(target).expect("target").hit_points;

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("high side hit should resolve");
    assert_eq!(event.target, target);
    assert!(!event.penetrated, "85 mm should bounce from 90 mm T-55 turret side armor");
    assert_eq!(state.tank(target).expect("target").hit_points, target_hp);
}

#[test]
fn off_center_side_hit_on_long_hull_still_uses_side_armor() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::new(-55.0, 0.0, 2.8));
    let target = state.spawn_tank(TeamId(2), TankSpec::t55a(), Vec3::ZERO);
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.yaw_rad = PI / 2.0;
        // Drop the shot to ~0.45 m — the lower tub side. This far forward (z = 2.8, near the
        // bow) the glacis has already cut the upper hull away above the 0.55 m sponson fold,
        // and the deck sits at only 1.30 m: a muzzle-height shell honestly clears the bow
        // entirely (the old model's tall box caught it in air).
        shooter.gun_pitch_rad = -0.028;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
    }

    run_until_shell_resolved(&mut state, shooter);

    let event = state.damage_events().last().expect("off-center side hit should resolve");
    assert_eq!(event.target, target);
    assert_eq!(event.armor_facing, ArmorFacing::HullSide);
    assert_eq!(event.armor_zone, ArmorZone::HullSide);
    assert_eq!(event.module, Some(ModuleSlot::Suspension));
}

#[test]
fn long_range_hit_uses_penetration_falloff() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target =
        state.spawn_tank(TeamId(2), TankSpec::tiger_i_ausf_e(), Vec3::new(0.0, 0.0, 900.0));
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = 0.007;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
        // Marginal at the muzzle: comfortably beats the plate at 100 m, dies to the velocity
        // the shell has bled by 900 m (pen(v) — see `ShellSpec::penetration_mm_at_distance`).
        shooter.spec.gun.shell.penetration_mm_at_100m = 100.0;
    }
    state.tank_mut(target).expect("target").yaw_rad = PI;
    let target_hp = state.tank(target).expect("target").hit_points;
    let step = FixedTimestep::from_hz(60);

    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..90 {
        state.apply_commands(&[], step);
        if !state.damage_events().is_empty() {
            break;
        }
    }

    let event = state.damage_events().last().expect("long-range hit should resolve");
    assert_eq!(event.target, target);
    assert!(!event.penetrated, "100 mm at 100 m should fall below Tiger I front at 900 m");
    assert_eq!(state.tank(target).expect("target").hit_points, target_hp);
}

/// Point-blank honesty: nose-to-nose the barrel tip sits INSIDE the enemy's armor volume, and
/// the shot must still connect — at the muzzle, against the plate the barrel poked through —
/// instead of tunnelling out the far side and sailing on. (Blueprint volumes used to report
/// start-inside segments as no hit; both duelists would dump whole magazines through each other.)
#[test]
fn point_blank_muzzle_inside_the_enemy_still_strikes_it() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t55a(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, 6.0));
    state.tank_mut(target).expect("target").yaw_rad = PI;

    // Precondition, so the test keeps meaning something if specs move: the muzzle really sits
    // inside the enemy turret volume's plan (bumper distance, level gun at dome height).
    let muzzle = state.tank(shooter).expect("shooter").muzzle_world_position();
    let hitbox = &state.tank(target).expect("target").spec.hitbox;
    let local_z = (state.tank(target).expect("target").position.z - muzzle.z).abs();
    assert!(
        local_z < hitbox.turret_half_length_m,
        "muzzle must start inside the enemy turret plan: {local_z} vs {}",
        hitbox.turret_half_length_m
    );

    // Point-blank resolves within the very tick that fires (events clear each tick), so probe
    // right after each step instead of using run_until_shell_resolved.
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..30 {
        if !state.damage_events().is_empty() || !state.shell_impacts().is_empty() {
            break;
        }
        state.apply_commands(&[], step);
    }

    assert!(state.shells().is_empty(), "the shot must not tunnel out and sail on");
    let event = state.damage_events().last().expect("point-blank shot must register an impact");
    assert_eq!(event.target, target);
    assert!(
        event.hit_position.distance(muzzle) < 0.75,
        "the strike lands at the muzzle, not extrapolated behind it or on the far side: \
         hit {:?} vs muzzle {muzzle:?}",
        event.hit_position
    );
}

fn run_until_shell_resolved(state: &mut SimulationState, shooter: game_core::TankId) {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, fire_command())], step);
    for _ in 0..30 {
        state.apply_commands(&[], step);
        if state.shells().is_empty() && !state.damage_events().is_empty() {
            return;
        }
    }
    panic!("shell should resolve against target");
}

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

fn ridge_heightmap() -> HeightMap {
    let mut samples = vec![0.0; 40 * 40];
    for x in 0..40 {
        samples[10 * 40 + x] = 6.0;
    }
    HeightMap::new(40, 40, 1.0, samples).expect("ridge heightmap")
}
