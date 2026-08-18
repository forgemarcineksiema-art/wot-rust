//! Locks the ricochet continuation: a shell that glances off a plate does not despawn — it
//! mirrors about the plate, keeps flying slower and blunted, and the NEXT surface resolves it
//! for good. One skip per shell, ever.

use std::f32::consts::PI;

use game_core::{TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

fn fire_command() -> TankCommand {
    TankCommand { fire: true, ..TankCommand::idle() }
}

#[test]
fn a_glance_off_the_side_keeps_the_shell_flying_blunted() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    // Target nearly parallel to the shot line, offset so the shell grazes its side plate at
    // ~75° — a guaranteed ricochet for a 100 mm round on an 80 mm plate. The lateral offset is
    // derived from the live hitbox so the side plane sits a few centimeters inside the shot
    // line, and the gun is pitched down to strike side-band height, not the deck.
    let tilt = 0.25_f32;
    let hitbox = TankSpec::t54_1951().hitbox;
    let target_x = -(hitbox.half_width_m * tilt.cos()) + 0.06;
    // Lower the target slightly so the 100 mm projectile body's bottom clears the track-band roof
    // and the intended grazing contact remains the hull side plate.
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(target_x, -0.2, 40.0));
    state.tank_mut(target).expect("target").yaw_rad = PI + tilt;
    state.tank_mut(shooter).expect("shooter").gun_pitch_rad = -0.018;
    let muzzle_speed = TankSpec::t54_1951().gun.shell.muzzle_velocity_mps;
    let step = FixedTimestep::from_hz(60);

    state.apply_commands(&[(shooter, fire_command())], step);
    let mut first_event = None;
    for _ in 0..240 {
        if let Some(event) = state.damage_events().first() {
            first_event = Some(*event);
            break;
        }
        state.apply_commands(&[], step);
    }

    let event = first_event.expect("the graze connects with the target");
    assert!(event.ricocheted, "a ~75° graze must ricochet, got {event:?}");
    assert!(!event.penetrated);
    assert_eq!(event.damage_hp, 0);

    // The shell survives its ricochet: still in flight, marked, slower, and blunted.
    assert_eq!(state.shells().len(), 1, "the glance keeps the shell flying");
    let shell = state.shells()[0];
    assert!(shell.ricocheted_once);
    assert!(
        shell.velocity_mps.length() < muzzle_speed * 0.85,
        "the skip bleeds speed: {} vs muzzle {muzzle_speed}",
        shell.velocity_mps.length()
    );
    assert!(
        shell.shell.penetration_mm_at_100m
            < TankSpec::t54_1951().gun.shell.penetration_mm_at_100m * 0.7,
        "the blunted round carries less penetration"
    );

    // And it is a one-time grace: the shell eventually resolves for good (here: ages out with
    // no terrain), never skipping forever.
    for _ in 0..400 {
        state.apply_commands(&[], step);
    }
    assert!(state.shells().is_empty(), "a ricocheted shell still dies for good");
}

/// The brittle-core rule (Amunicja 3.0 A4): where full-bore AP skips, a tungsten core SHATTERS
/// — same graze, same plate, but the round dies on the outer face with no deflection and no
/// continued flight. The taxonomy the player learns: penetration / bounce / shatter / near-pen
/// rattle, and the "gold" round's doctrine cost is that it never gets the skip.
#[test]
fn apcr_on_a_steep_plate_shatters_instead_of_skipping() {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    // The exact geometry of the AP glance above — only the projectile differs: a synthetic
    // 100 mm tungsten core, so the ricochet decision lands identically.
    let tilt = 0.25_f32;
    let hitbox = TankSpec::t54_1951().hitbox;
    let target_x = -(hitbox.half_width_m * tilt.cos()) + 0.06;
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(target_x, -0.2, 40.0));
    state.tank_mut(target).expect("target").yaw_rad = PI + tilt;
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = -0.018;
        shooter.spec.gun.shell = game_core::ShellSpec::apcr(100.0, 895.0, 170.0, 170);
    }
    let step = FixedTimestep::from_hz(60);

    state.apply_commands(&[(shooter, fire_command())], step);
    let mut first_event = None;
    for _ in 0..240 {
        if let Some(event) = state.damage_events().first() {
            first_event = Some(*event);
            break;
        }
        state.apply_commands(&[], step);
    }

    let event = first_event.expect("the graze connects with the target");
    assert!(event.ricocheted, "the ricochet DECISION is unchanged — only what follows differs");
    assert!(!event.penetrated);
    assert_eq!(event.damage_hp, 0);
    assert!(
        state.shells().is_empty(),
        "a shattered round never flies on: the core died on the plate it failed to skid off"
    );
}
