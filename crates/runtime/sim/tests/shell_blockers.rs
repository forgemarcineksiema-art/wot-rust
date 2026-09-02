//! Shells are absorbed by friendly hulls and wrecks (no damage, no pass-through), and every
//! absorbed shell emits a [`game_core::ShellImpact`] so the shot never vanishes silently.
//! Includes the negative locks per the contact-test engineering rule: a clean enemy hit emits
//! damage and **no** impact event, and absorption must not invent damage.

use std::f32::consts::PI;

use game_core::{ImpactSurface, TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};
use terrain::HeightMap;

const STEP_HZ: u32 = 60;

fn flat() -> HeightMap {
    HeightMap::flat(128, 128, 4.0, 0.0).expect("flat terrain")
}

/// Shooter at z=20 firing flat down +z with zeroed dispersion.
fn shooter_state() -> (SimulationState, TankId) {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 20.0));
    {
        let tank = state.tank_mut(shooter).expect("shooter");
        tank.aim_dispersion_mrad = 0.0;
        tank.spec.gun.dispersion_mrad = 0.0;
    }
    (state, shooter)
}

fn fire_and_settle(state: &mut SimulationState, shooter: TankId) {
    let step = FixedTimestep::from_hz(STEP_HZ);
    let terrain = flat();
    state.apply_commands_on_terrain(
        &[(shooter, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );
    for _ in 0..120 {
        if state.shells().is_empty() {
            break;
        }
        state.apply_commands_on_terrain(&[], step, &terrain);
    }
}

#[test]
fn friendly_hull_absorbs_the_shell_without_damage() {
    let (mut state, shooter) = shooter_state();
    let ally = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 50.0));
    let enemy = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 90.0));
    state.tank_mut(enemy).expect("enemy").yaw_rad = PI;
    let ally_hp = state.tank(ally).expect("ally").hit_points;
    let enemy_hp = state.tank(enemy).expect("enemy").hit_points;

    fire_and_settle(&mut state, shooter);

    assert!(state.damage_events().is_empty(), "a blocked shell must not damage anyone");
    assert_eq!(state.tank(ally).expect("ally").hit_points, ally_hp, "no friendly fire damage");
    assert_eq!(state.tank(enemy).expect("enemy").hit_points, enemy_hp, "enemy stays protected");

    let impact = state.shell_impacts().last().expect("absorption must emit impact feedback");
    assert_eq!(impact.owner, Some(shooter));
    assert_eq!(impact.surface, ImpactSurface::Hull);
    // The ally sits at z=50 with half_length 3.2: the shell dies on its near face, not beyond.
    assert!(
        (45.0..=50.0).contains(&impact.position.z),
        "impact should sit on the ally's near face, got z {}",
        impact.position.z
    );
}

#[test]
fn wreck_blocks_the_shell_and_protects_the_target_behind_it() {
    let (mut state, shooter) = shooter_state();
    let wreck = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 50.0));
    let enemy = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 90.0));
    state.tank_mut(wreck).expect("wreck").hit_points = 0;
    state.tank_mut(enemy).expect("enemy").yaw_rad = PI;
    let enemy_hp = state.tank(enemy).expect("enemy").hit_points;

    fire_and_settle(&mut state, shooter);

    assert!(state.damage_events().is_empty(), "a wreck absorbs the shell without damage");
    assert_eq!(state.tank(enemy).expect("enemy").hit_points, enemy_hp, "wreck is hard cover");
    let impact = state.shell_impacts().last().expect("wreck absorption emits impact feedback");
    assert_eq!(impact.surface, ImpactSurface::Hull);
    assert!(impact.position.z < 50.0, "shell must die on the wreck's near side");
}

#[test]
fn clean_enemy_hit_emits_damage_and_its_own_hull_impact() {
    let (mut state, shooter) = shooter_state();
    let enemy = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(100.0, 0.0, 90.0));
    state.tank_mut(enemy).expect("enemy").yaw_rad = PI;

    fire_and_settle(&mut state, shooter);

    let event = state.damage_events().last().expect("clean shot must damage the enemy");
    assert_eq!(event.target, enemy);
    // Inny Poziom S9: the strike is damage feedback AND the shell's own death on the hull —
    // one impact, surface Hull, at the plate's hit point, the same shell as the event — so the
    // client can draw an HE round's blast from it. Never an absorption on a blocker.
    assert_eq!(state.shell_impacts().len(), 1, "one hull impact beside the damage event");
    let impact = state.shell_impacts()[0];
    assert_eq!(impact.surface, ImpactSurface::Hull);
    assert_eq!(impact.shell_id, event.shell_id.expect("the event names its shell"));
    assert!((impact.position - event.hit_position).length() < 1.0e-3, "at the plate's hit point");
}

#[test]
fn shot_into_the_ground_emits_a_terrain_impact() {
    let (mut state, shooter) = shooter_state();
    let step = FixedTimestep::from_hz(STEP_HZ);
    let terrain = flat();

    // Depress the gun fully (~-0.14 rad), then fire into the dirt ahead.
    for _ in 0..30 {
        state.apply_commands_on_terrain(
            &[(shooter, TankCommand { gun_pitch_delta: -1.0, ..TankCommand::idle() })],
            step,
            &terrain,
        );
    }
    state.apply_commands_on_terrain(
        &[(shooter, TankCommand { fire: true, ..TankCommand::idle() })],
        step,
        &terrain,
    );
    let mut impact = None;
    for _ in 0..120 {
        if let Some(found) = state.shell_impacts().last() {
            impact = Some(*found);
            break;
        }
        state.apply_commands_on_terrain(&[], step, &terrain);
    }

    let impact = impact.expect("a ground shot must report where it landed");
    assert_eq!(impact.surface, ImpactSurface::Terrain);
    assert_eq!(impact.owner, Some(shooter));
    assert!(impact.position.z > 20.0, "impact lands ahead of the muzzle");
    assert!(impact.position.y <= 0.1, "impact sits at ground level, got y {}", impact.position.y);
    // Protocol v17: the impact says WHAT died here, so presentation can voice HE as a blast.
    // This shot fired the tank's currently selected shell; the wire must carry its real type,
    // not a default placeholder.
    let selected = {
        let tank = state.tank(shooter).expect("shooter");
        tank.spec.gun.ammo_options()[tank.selected_ammo as usize].shell_type
    };
    assert_eq!(impact.shell_type, selected);
}
