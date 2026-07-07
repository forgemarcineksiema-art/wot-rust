//! Locks the v19 impact-presentation truth end to end: a real shell fired into an enemy hull
//! produces a `DamageEvent` carrying the struck plate's true outward world normal and the
//! shell's heading. This is what lets the client seat the impact mark flush on the visual armor
//! (destruction program phase 1) instead of guessing a cardinal facing normal.

use std::f32::consts::PI;

use game_core::TeamId;
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

/// Fire from a T-55 into a T-54 dead ahead, facing back, and return the first shell damage event.
fn front_hit_event() -> game_core::DamageEvent {
    let mut state = SimulationState::new();
    let shooter =
        state.spawn_tank(TeamId(1), game_core::VehicleKind::T55A.spec(), Vec3::new(0.0, 0.0, 0.0));
    // Enemy 20 m ahead, hull yawed to face the shooter so its front armor takes the shot.
    let _target = state.spawn_tank_with_yaw(
        TeamId(2),
        game_core::VehicleKind::T54_1951.spec(),
        Vec3::new(0.0, 0.0, 20.0),
        PI,
    );
    let step = FixedTimestep::from_hz(60);

    // The muzzle already sits downrange and the shell steps ~15 m/tick, so the strike lands on
    // the fire tick itself — check it before, and across, the following ticks (damage events are
    // cleared each tick, so a found event must be read the tick it happens).
    state.apply_commands(&[(shooter, TankCommand { fire: true, ..TankCommand::idle() })], step);
    if let Some(event) = shell_event(&state) {
        return event;
    }
    for _ in 0..120 {
        state.apply_commands(&[(shooter, TankCommand::idle())], step);
        if let Some(event) = shell_event(&state) {
            return event;
        }
    }
    panic!("the shell never struck the enemy hull");
}

fn shell_event(state: &SimulationState) -> Option<game_core::DamageEvent> {
    state.damage_events().iter().find(|event| event.cause == game_core::DamageCause::Shell).copied()
}

#[test]
fn a_frontal_shell_hit_reports_a_true_outward_plate_normal() {
    let event = front_hit_event();

    // The heading is the shell's own travel: straight downrange along +Z.
    assert!(
        (event.shell_direction - Vec3::Z).length() < 0.05,
        "shell heading is downrange, got {:?}",
        event.shell_direction
    );

    // The plate normal is a real, finite, unit-ish outward vector.
    let normal = event.plate_normal;
    assert!(normal.is_finite() && normal.length() > 0.5, "a real plate normal, got {normal:?}");

    // Outward means it faces the incoming shell: the target's front is turned toward the shooter
    // (down -Z), so the normal opposes the +Z shell.
    assert!(
        normal.dot(event.shell_direction) < 0.0,
        "an outward plate normal opposes the incoming shell, got n={normal:?}"
    );
    assert!(normal.z < 0.0, "the struck front plate faces back toward the shooter, got {normal:?}");
}
