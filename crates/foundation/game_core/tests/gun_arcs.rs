//! Gun arcs are authored, and the arc is an identity.
//!
//! Until 2026-08-02 every gun in the catalog but the T-54's ran the fleet placeholder
//! (−8° / +20.1°) — the default the `GunSpec` serde fills in. Depression is *the* hull-down stat:
//! it decides who can fight from a ridge and who must fight on the flat, and a fleet on one
//! placeholder had its most tank-like axis of identity flattened to a constant. The sourced arcs
//! live in `docs/ammunition.md` ("Gun arcs").

use game_core::VehicleKind;

/// The old fleet default. A gun carrying exactly this pair is a gun nobody authored.
const PLACEHOLDER: (f32, f32) = (8.0, 20.1);

#[test]
fn no_real_gun_runs_the_placeholder_arc() {
    let mut checked = 0;
    for kind in VehicleKind::ALL {
        for gun in kind.gun_options() {
            checked += 1;
            assert!(
                (gun.spec.depression_deg, gun.spec.elevation_deg) != PLACEHOLDER,
                "{}: still on the fleet placeholder −8/+20.1 — author its real arc",
                gun.spec.name
            );
        }
    }
    assert!(checked >= 11, "every gun in the fleet must be examined, saw {checked}");
}

/// The spread IS the point. The IS-3 has the shallowest depression in the fleet and the
/// Centurion the deepest — the pike-nosed brawler that fights on the flat against the Western
/// turret that lives on a ridgeline. Locked as an ORDERING so a tuning pass cannot quietly
/// flatten the fleet back into one number.
#[test]
fn the_is3_fights_on_the_flat_and_the_centurion_owns_the_ridge() {
    let depression = |kind: VehicleKind| {
        kind.gun_options()
            .first()
            .map(|gun| gun.spec.depression_deg)
            .expect("every vehicle mounts a stock gun")
    };
    let is3 = depression(VehicleKind::IS3);
    let centurion = depression(VehicleKind::Centurion);
    for kind in VehicleKind::PLAYABLE {
        let this = depression(kind);
        assert!(is3 <= this, "{kind:?}: nothing may depress worse than the IS-3's −3");
        assert!(centurion >= this, "{kind:?}: nothing may out-depress the Centurion's −10");
    }
    assert!((is3 - 3.0).abs() < 1.0e-6, "the IS-3's −3 is the sourced figure");
    assert!((centurion - 10.0).abs() < 1.0e-6, "the Centurion's −10 is the sourced figure");
}

/// The clamps the sim actually uses come from these fields, so the arc is gameplay the moment it
/// is authored — no second wiring step exists to forget.
#[test]
fn the_pitch_clamps_are_the_authored_arc_in_radians() {
    let spec = VehicleKind::IS3.spec();
    let (min, max) = spec.gun_pitch_limits_rad();
    assert!((min + 3.0_f32.to_radians()).abs() < 1.0e-6, "min pitch is −depression");
    assert!((max - 20.0_f32.to_radians()).abs() < 1.0e-6, "max pitch is +elevation");
}
