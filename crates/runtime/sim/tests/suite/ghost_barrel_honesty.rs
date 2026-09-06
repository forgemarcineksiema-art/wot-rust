//! THE GUN IS A GHOST, AND THE ARMOUR IS NOT — P0.3 of `docs/contact-and-tracks-program.md`.
//!
//! The barrel takes part in no collision: it passes through hulls, walls and terrain, and that is
//! a decision, not an oversight (a five-metre lever on a thirty-six-tonne body in a planar solver
//! is a stability problem, and a player who could lever themselves off scenery would).
//!
//! The decision has one edge worth being sure about. Shells spawn at the visible muzzle
//! (`docs/combat-policy.md`), so a barrel pushed into somebody's hull spawns its round INSIDE
//! them — past the plate, on the wrong side of the armour that was supposed to stop it. If that
//! were how it resolved, shoving your gun into an enemy at a corner would be a way to delete their
//! frontal armour, which is the opposite of the honesty doctrine and would be nobody's idea of a
//! feature.
//!
//! `point_blank_muzzle_inside_the_enemy_still_strikes_it` in `combat_pipeline.rs` already locks
//! that such a shot CONNECTS instead of tunnelling out the far side. This locks the other half:
//! that it connects against the same plate, with the same steel, as the same shot taken from
//! across the field. Measured as a difference between two shots rather than against a written-down
//! millimetre count, so it keeps meaning something when the armour is re-authored.

use std::f32::consts::PI;

use game_core::{TankId, TankSpec, TeamId};
use glam::Vec3;
use sim::{FixedTimestep, SimulationState, TankCommand};

/// Far enough that the muzzle is nowhere near the target — an ordinary shot.
const ACROSS_THE_FIELD_M: f32 = 55.0;

/// Nose to nose with the barrel buried in the target: hulls 8 m apart do not touch (3.27 m of
/// half-length each leaves 1.47 m of air), but the D-10T reaches z = 5.95, so the muzzle sits
/// 1.2 m PAST the target's front face and inside its armour volume.
const BARREL_BURIED_M: f32 = 8.0;

#[test]
fn a_muzzle_inside_the_enemy_still_meets_the_plate_it_pushed_through() {
    let buried = fire_at(BARREL_BURIED_M);
    let across = fire_at(ACROSS_THE_FIELD_M);
    println!("barrel buried at {BARREL_BURIED_M} m: {buried}");
    println!("across the field at {ACROSS_THE_FIELD_M} m: {across}");

    // Precondition, so this keeps testing what it says if the fleet's geometry moves: the muzzle
    // really is inside the target at the close range, and really is not at the far one.
    assert!(buried.muzzle_inside_target, "the close shot must start with the barrel buried");
    assert!(!across.muzzle_inside_target, "the far shot must start clear of the target");

    // The same plate, on the same facing, at the same angle. Nothing about where the round
    // STARTED may change which steel it has to cross.
    assert_eq!(
        buried.armor_zone, across.armor_zone,
        "a buried muzzle resolved against a different zone than the same shot from range"
    );
    assert_eq!(buried.armor_facing, across.armor_facing);
    assert!(
        (buried.nominal_armor_mm - across.nominal_armor_mm).abs() < 1.0,
        "the plate lost {:.1} mm of nominal thickness by being shot from inside: {:.1} vs {:.1}",
        across.nominal_armor_mm - buried.nominal_armor_mm,
        buried.nominal_armor_mm,
        across.nominal_armor_mm
    );
    assert!(
        buried.nominal_armor_mm > 1.0,
        "a shot from inside met no plate at all ({:.1} mm) — the armour was skipped",
        buried.nominal_armor_mm
    );
    assert!(
        (buried.effective_armor_mm - across.effective_armor_mm).abs()
            < across.effective_armor_mm * 0.05,
        "the effective armour changed with the muzzle's position: {:.1} vs {:.1} mm",
        buried.effective_armor_mm,
        across.effective_armor_mm
    );

    // What SHOULD differ is the round: less air crossed means less velocity bled, so a buried
    // muzzle hits harder. That is `pen(v)` doing its job, not the plate going missing.
    assert!(
        buried.shell_penetration_mm >= across.shell_penetration_mm,
        "point blank must not arrive weaker than a shot from 55 m: {:.1} vs {:.1} mm",
        buried.shell_penetration_mm,
        across.shell_penetration_mm
    );
}

struct Shot {
    muzzle_inside_target: bool,
    armor_zone: game_core::ArmorZone,
    armor_facing: game_core::ArmorFacing,
    nominal_armor_mm: f32,
    effective_armor_mm: f32,
    shell_penetration_mm: f32,
}

impl std::fmt::Display for Shot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}/{:?}, {:.0} mm nominal -> {:.0} mm effective, round arrives with {:.0} mm",
            self.armor_zone,
            self.armor_facing,
            self.nominal_armor_mm,
            self.effective_armor_mm,
            self.shell_penetration_mm
        )
    }
}

/// One level shot at a T-54 standing `range_m` away, nose on. Dispersion is zeroed so the two
/// shots differ in exactly one thing: how far the muzzle is from the plate.
fn fire_at(range_m: f32) -> Shot {
    let mut state = SimulationState::new();
    let shooter = state.spawn_tank(TeamId(1), TankSpec::t54_1951(), Vec3::ZERO);
    let target = state.spawn_tank(TeamId(2), TankSpec::t54_1951(), Vec3::new(0.0, 0.0, range_m));
    state.tank_mut(target).expect("target").yaw_rad = PI;
    {
        let shooter = state.tank_mut(shooter).expect("shooter");
        shooter.gun_pitch_rad = 0.0;
        shooter.aim_dispersion_mrad = 0.0;
        shooter.spec.gun.dispersion_mrad = 0.0;
    }

    let muzzle = state.tank(shooter).expect("shooter").muzzle_world_position();
    let target_state = state.tank(target).expect("target");
    let hitbox = target_state.spec.hitbox;
    let muzzle_inside_target = (muzzle.z - target_state.position.z).abs() < hitbox.half_length_m
        && (muzzle.x - target_state.position.x).abs() < hitbox.half_width_m;

    let event = resolve_one_shot(&mut state, shooter);
    assert_eq!(event.target, target, "the shot at {range_m} m must reach the target");
    Shot {
        muzzle_inside_target,
        armor_zone: event.armor_zone,
        armor_facing: event.armor_facing,
        nominal_armor_mm: event.nominal_armor_mm,
        effective_armor_mm: event.effective_armor_mm,
        shell_penetration_mm: event.shell_penetration_mm,
    }
}

/// Fire and step until the round resolves. Point blank resolves inside the firing tick and the
/// event buffers clear every tick, so the result is captured the moment it appears.
fn resolve_one_shot(state: &mut SimulationState, shooter: TankId) -> game_core::DamageEvent {
    let step = FixedTimestep::from_hz(60);
    state.apply_commands(&[(shooter, TankCommand { fire: true, ..TankCommand::idle() })], step);
    for _ in 0..120 {
        if let Some(event) = state.damage_events().last() {
            return *event;
        }
        state.apply_commands(&[], step);
    }
    panic!("the shot never resolved");
}
