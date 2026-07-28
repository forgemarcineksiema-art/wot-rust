//! Gun depression is a per-vehicle property, and one of the sharpest levers a tank has.
//!
//! The whole fleet used to clamp against one hard-coded pair — `-0.14 rad / +0.35 rad`, i.e.
//! about -8°/+20°. That handed the T-54 a ridge-fighting ability it never had: its documented
//! arc is **-5°/+18°**, and the reason is structural — the low cast dome that makes the tank
//! hard to hit is exactly what leaves the breech no room to drop.
//!
//! A tank that cannot depress must expose more hull to shoot down a slope. That is the trade the
//! real vehicle made, and now the game makes it too.

use game_core::{TankSpec, VehicleKind};
use sim::{AimingState, TankCommand, step_aiming};

fn spec(kind: VehicleKind) -> TankSpec {
    kind.spec()
}

/// Drive the gun to its stop in one direction and report where it settled.
fn pitch_at_stop(spec: &TankSpec, direction: f32) -> f32 {
    let mut aiming = AimingState::default();
    let command = TankCommand {
        throttle: 0.0,
        steer: 0.0,
        brake: 0.0,
        turret_yaw_delta: 0.0,
        gun_pitch_delta: direction,
        fire: false,
        select_ammo: None,
    };
    for _ in 0..600 {
        step_aiming(&mut aiming, spec, command, 1.0 / 60.0);
    }
    aiming.gun_pitch_rad
}

#[test]
fn the_t54_stops_at_its_documented_five_degrees_of_depression() {
    let t54 = spec(VehicleKind::T54_1951);
    let depression = pitch_at_stop(&t54, -1.0).to_degrees();
    let elevation = pitch_at_stop(&t54, 1.0).to_degrees();

    assert!(
        (depression + 5.0).abs() < 0.01,
        "the T-54 depresses 5 degrees, not the fleet's old 8 — got {depression:.2}"
    );
    assert!(
        (elevation - 18.0).abs() < 0.01,
        "the T-54 elevates 18 degrees, not the fleet's old 20 — got {elevation:.2}"
    );
}

#[test]
fn the_arc_is_read_from_the_installed_gun_not_from_a_constant() {
    let mut t54 = spec(VehicleKind::T54_1951);
    let (min_before, _) = t54.gun_pitch_limits_rad();

    // Fit a gun with a different mount and the tank's reach changes with it.
    t54.gun.depression_deg = 10.0;
    let (min_after, _) = t54.gun_pitch_limits_rad();
    assert!(
        min_after < min_before,
        "a mount with more room must let the gun drop further: {min_after} vs {min_before}"
    );

    let reached = pitch_at_stop(&t54, -1.0).to_degrees();
    assert!(
        (reached + 10.0).abs() < 0.01,
        "the sim clamps against the INSTALLED gun's arc, got {reached:.2}"
    );
}

/// Every vehicle answers the question, and no vehicle silently keeps a nonsense arc.
#[test]
fn every_playable_vehicle_declares_a_sane_arc() {
    for kind in VehicleKind::PLAYABLE {
        let spec = spec(kind);
        let (min_pitch, max_pitch) = spec.gun_pitch_limits_rad();
        assert!(
            min_pitch < 0.0 && max_pitch > 0.0,
            "{kind:?}: a gun must be able to point both below and above the horizon"
        );
        let depression = -min_pitch.to_degrees();
        let elevation = max_pitch.to_degrees();
        assert!(
            (2.0..=15.0).contains(&depression),
            "{kind:?}: {depression:.1} degrees of depression is outside anything a tank of this \
             era carried — check the dossier before accepting it"
        );
        assert!(
            (10.0..=40.0).contains(&elevation),
            "{kind:?}: {elevation:.1} degrees of elevation is outside the era's range"
        );
    }
}

/// The hull-down consequence, stated as a test: with less depression, the same crest forces the
/// tank to show more of itself. This is the whole reason the number matters.
#[test]
fn less_depression_means_more_hull_exposed_over_the_same_crest() {
    let t54 = spec(VehicleKind::T54_1951);
    let mut generous = t54.clone();
    generous.gun.depression_deg = 10.0;

    // A close crest, the case depression actually decides: a target 3 m below the gun line,
    // 30 m out. How far the gun line falls SHORT of it, held at the stop, is how far the tank
    // must creep onto the skyline before it can shoot at all.
    const RANGE_M: f32 = 30.0;
    const TARGET_DROP_M: f32 = 3.0;
    let exposure = |spec: &TankSpec| {
        let (min_pitch, _) = spec.gun_pitch_limits_rad();
        let reach = (-min_pitch).tan() * RANGE_M;
        (TARGET_DROP_M - reach).max(0.0)
    };

    assert!(
        exposure(&t54) > exposure(&generous),
        "the -5 tank must give up more cover than the -10 one: {:.2} m vs {:.2} m",
        exposure(&t54),
        exposure(&generous)
    );
}
