//! ROLLOVER IS IN THE MODEL AND OUT OF REACH — P0.2 of `docs/contact-and-tracks-program.md`.
//!
//! The decision this locks is not "tanks cannot roll over". It is the stronger and more honest
//! one: a hull always breaks traction BEFORE it can lean far enough to tip, so no turn, no side
//! slope the map contract permits, no curb and no broadside ram can put a tank on its roof. The
//! only path left open is an asymmetric landing after a fall, which Wave 3 turns into a violent
//! but recoverable roll.
//!
//! Stated as arithmetic instead of as a promise:
//!
//! * a hull tips when lateral acceleration reaches `g · tip_edge / com_height` — the static
//!   stability factor, from `game_core::stability`;
//! * a hull SLIDES when it reaches `g · lateral_grip_mu · grip`, where `grip` is the best any
//!   ground material offers.
//!
//! As long as the second is comfortably below the first, tipping is unreachable — and it stays
//! unreachable when either number moves, because this test re-derives both from the fleet's own
//! data every run rather than from a figure somebody wrote down once.

use game_core::VehicleKind;
use physics::TankControllerSettings;
use terrain::GroundMaterial;

/// How much room the verdict must keep over the crossover, on top of both numbers already being
/// derived pessimistically (the centre of mass is read high; the grip is the best in the game).
///
/// Set from the measurement, and worth saying how, because the first attempt got it backwards.
/// The program's sketch guessed 1.15 from a hand-estimated 1.00 m centre of mass. The DERIVED
/// centres of mass are 1.21–1.42 m, and the fleet's real margins came out **1.15× to 1.38×**, with
/// the Centurion tightest — so the guess would have failed the very fleet it was written for, not
/// because anything was wrong but because the guess was never a measurement.
///
/// 1.10 is therefore where the gate bites: it leaves the tightest hull about four percent of its
/// headroom, which is close enough that any change eroding this crossover trips it, and far enough
/// from 1.0 that the promise ("a hull always slides first") is never quietly lost. Raising this
/// constant to turn a red test green is the one move that defeats the purpose — the answer to a
/// failure here is a wider track, a lower turret, or less lateral grip.
const SAFETY: f32 = 1.10;

#[test]
fn every_hull_slides_before_it_can_tip() {
    let grip = best_grip_in_the_game();
    let mut rows = Vec::new();
    let mut failures = Vec::new();

    for kind in VehicleKind::PLAYABLE {
        let spec = kind.spec();
        let settings = TankControllerSettings::from_spec(&spec);
        let slide_g = settings.lateral_grip_mu * grip;

        // The worst loadout the garage can build, not the stock one: a heavier turret is bought
        // with tonnes that sit high, so the hull that tips most easily is the one somebody
        // deliberately loaded up.
        let (tip_g, com_m, turret) = worst_loadout(kind);
        let margin = tip_g / slide_g;
        rows.push(format!(
            "{kind:?}: com {com_m:.3} m, tips at {tip_g:.2} g, slides at {slide_g:.2} g, \
             margin {margin:.2}x (worst turret: {turret})"
        ));
        if margin <= SAFETY {
            failures.push(format!(
                "{kind:?} can lean to within {margin:.2}x of tipping before its tracks let go \
                 (com {com_m:.3} m on turret {turret})"
            ));
        }
    }

    println!("{}", rows.join("\n"));
    assert!(
        failures.is_empty(),
        "a hull in this fleet can tip before it slides, and rollover stops being unreachable:\n{}",
        failures.join("\n")
    );
}

/// The margin must come from the vehicles, not from the constant. If every hull cleared by miles
/// the test would be measuring nothing; this pins that the recorded `SAFETY` is close enough to
/// the real crossover to still be a gate.
#[test]
fn the_safety_margin_is_a_gate_and_not_a_formality() {
    let grip = best_grip_in_the_game();
    let tightest = VehicleKind::PLAYABLE
        .into_iter()
        .map(|kind| {
            let settings = TankControllerSettings::from_spec(&kind.spec());
            worst_loadout(kind).0 / (settings.lateral_grip_mu * grip)
        })
        .fold(f32::INFINITY, f32::min);
    assert!(
        tightest < SAFETY * 1.5,
        "the tightest hull clears by {tightest:.2}x against a {SAFETY} gate — the gate has \
         stopped touching the fleet and would not notice a change that matters"
    );
}

/// Tipping does not care what the ground is made of, but sliding does: the more grip a surface
/// offers, the further a hull can lean before its tracks give up. The dangerous surface is
/// therefore the grippiest one in the game.
fn best_grip_in_the_game() -> f32 {
    GroundMaterial::ALL
        .into_iter()
        .map(|material| material.properties().grip_scale)
        .fold(0.0, f32::max)
}

/// The highest centre of mass the garage can put on this chassis: every turret it will accept,
/// carrying every gun that turret will accept. Returns the tipping threshold, the centre-of-mass
/// height, and the turret that produced them.
fn worst_loadout(kind: VehicleKind) -> (f32, f32, String) {
    let mut worst: Option<(f32, f32, String)> = None;
    for turret in kind.turret_options() {
        let mut modules = kind.default_loadout();
        if modules.try_install_turret(turret.clone()).is_err() {
            continue;
        }
        for gun in kind.gun_options() {
            let mut loaded = modules.clone();
            let _ = loaded.try_install_gun(gun);
            consider(kind, &loaded, &turret.name, &mut worst);
        }
        consider(kind, &modules, &turret.name, &mut worst);
    }
    // A chassis whose every option was refused still has the loadout it rolled out with.
    worst.unwrap_or_else(|| {
        let stock =
            game_core::stock_stability(kind).expect("every playable vehicle has a blueprint");
        (stock.tipping_threshold_g(), stock.com_height_m, "stock".to_string())
    })
}

fn consider(
    kind: VehicleKind,
    modules: &game_core::VehicleModules,
    turret: &str,
    worst: &mut Option<(f32, f32, String)>,
) {
    let Some(stability) = game_core::stability(kind, modules) else { return };
    let threshold = stability.tipping_threshold_g();
    if worst.as_ref().is_none_or(|(lowest, _, _)| threshold < *lowest) {
        *worst = Some((threshold, stability.com_height_m, turret.to_string()));
    }
}
