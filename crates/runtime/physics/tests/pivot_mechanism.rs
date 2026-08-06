//! A HULL TURNS ABOUT WHAT ITS GEARBOX LETS IT TURN ABOUT — P4.5 of
//! `docs/contact-and-tracks-program.md`.
//!
//! Every vehicle in this game used to pivot the same way, because the drive model was handed one
//! turn rate per suspension module and no reason to ask what produced it. Most of these tanks could
//! not counter-rotate their tracks at all.
//!
//! The split is by design school and not by era, which is the finding that corrected the plan: the
//! 1942 Tiger I turns about its own centre and the 1951 T-54 does not. Two of eight vehicles are
//! regenerative — the Tiger I's Argus unit, derived from the Merritt-Brown type, and the
//! Centurion's Merritt-Brown Z51R triple differential. The other six can only slow or stop the
//! inner belt, so they swing about it: half the rate, and they walk forward while they do it.
//!
//! Sources are cited per vehicle in `game_core::SteeringKind::for_vehicle`.

use game_core::{SteeringKind, VehicleKind};
use physics::{
    TankControlInput, TankControllerSettings, TankKinematicState, TerrainContact,
    step_custom_tank_controller_on_contact,
};

const DT: f32 = 1.0 / 60.0;

/// Spin a stationary hull for three seconds and report how far it turned and how far it walked.
fn pivot(kind: VehicleKind) -> (f32, f32) {
    let spec = kind.spec();
    let settings = TankControllerSettings::from_spec(&spec);
    let mut state = TankKinematicState::default();
    let input = TankControlInput { throttle: 0.0, steer: 1.0, brake: 0.0 };
    for _ in 0..180 {
        step_custom_tank_controller_on_contact(
            &mut state,
            input,
            &settings,
            TerrainContact::flat(0.0),
            DT,
        );
    }
    (state.yaw_rad, state.position.length())
}

#[test]
fn a_gearbox_that_cannot_reverse_a_track_swings_the_hull_about_it() {
    println!("{:<12} {:<14} {:>10} {:>12}", "vehicle", "gearbox", "turned", "walked");
    let mut regenerative = Vec::new();
    let mut braked = Vec::new();
    for kind in VehicleKind::PLAYABLE {
        let (turned, walked) = pivot(kind);
        let steering = SteeringKind::for_vehicle(kind);
        println!(
            "{:<12} {:<14} {:>7.0}°/3s {:>10.2} m",
            format!("{kind:?}"),
            format!("{steering:?}"),
            turned.to_degrees(),
            walked
        );
        if steering.counter_rotates() { &mut regenerative } else { &mut braked }
            .push((kind, turned, walked));
    }

    for (kind, _, walked) in &regenerative {
        assert!(
            *walked < 0.01,
            "{kind:?} counter-rotates, so its centre must stay put — it moved {walked:.3} m"
        );
    }
    for (kind, turned, walked) in &braked {
        assert!(
            *walked > 0.5,
            "{kind:?} swings about a belt, so it must walk itself round — it moved {walked:.3} m"
        );
        // Still a pivot, and measured against what half this hull's own authored rate would
        // give over the window rather than against a threshold somebody picked.
        let halved = TankControllerSettings::from_spec(&kind.spec()).turn_rate_rad_s * 0.5;
        assert!(
            turned.abs() >= halved * 3.0 * 0.7,
            "{kind:?} must still turn on the spot, just differently: {:.0}° in 3 s against a              halved rate of {:.2} rad/s",
            turned.to_degrees(),
            halved
        );
    }
    assert_eq!(regenerative.len(), 2, "only the Tiger I and the Centurion counter-rotate");
    assert_eq!(braked.len(), 6);
}

/// Half the track doing the work is half the rate. Compared between two hulls whose authored turn
/// rate is identical, so the difference can only be the gearbox.
#[test]
fn swinging_about_a_belt_costs_half_the_rate() {
    // The Tiger I and the IS-3 both carry an authored 0.58 rad/s, from opposite design schools.
    let tiger = TankControllerSettings::from_spec(&VehicleKind::TigerI.spec()).turn_rate_rad_s;
    let is3 = TankControllerSettings::from_spec(&VehicleKind::IS3.spec()).turn_rate_rad_s;
    assert!((tiger - is3).abs() < 1.0e-6, "this comparison needs the same authored rate");

    let (tiger_turned, _) = pivot(VehicleKind::TigerI);
    let (is3_turned, _) = pivot(VehicleKind::IS3);
    let ratio = is3_turned.abs() / tiger_turned.abs();
    println!(
        "same authored rate, different gearbox: Tiger I {:.0}°, IS-3 {:.0}° (ratio {ratio:.2})",
        tiger_turned.to_degrees(),
        is3_turned.to_degrees()
    );
    assert!(
        (0.45..=0.6).contains(&ratio),
        "a braked-belt pivot must cost about half the rate, got {ratio:.2}"
    );
}

/// Under power nothing changes. This PR is about the pivot, not about the drive model — a turn
/// taken at speed is a radius, and radii are Wave 4's business.
#[test]
fn steering_under_power_is_untouched() {
    for kind in [VehicleKind::T54_1951, VehicleKind::TigerI] {
        let spec = kind.spec();
        let settings = TankControllerSettings::from_spec(&spec);
        let mut state = TankKinematicState::default();
        let input = TankControlInput { throttle: 1.0, steer: 1.0, brake: 0.0 };
        for _ in 0..600 {
            step_custom_tank_controller_on_contact(
                &mut state,
                input,
                &settings,
                TerrainContact::flat(0.0),
                DT,
            );
        }
        let settled = state.yaw_rate_rad_s.abs();
        let commanded = settings.turn_rate_rad_s;
        println!("{kind:?} under power: {settled:.3} rad/s against an authored {commanded:.3}");
        assert!(
            (settled - commanded).abs() < 0.05,
            "{kind:?} lost its authored turn rate under power: {settled:.3} vs {commanded:.3}"
        );
    }
}
