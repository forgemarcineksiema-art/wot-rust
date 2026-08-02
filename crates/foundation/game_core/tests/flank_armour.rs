//! Side armour decides flank fights — through TRUE geometry, not through a facet's flat number.
//!
//! `docs/battle-first/fleet-numbers-audit.md` first claimed the opposite ("side armour is
//! decorative: every gun penetrates every side"). That was measured with
//! `effective_thickness_mm(HullSide, 0.0)`, which is nominal x weakspot with no geometry in it at
//! all — the plate's slope lives in `plate_normal`, and the impact angle is taken against that 3D
//! normal. Measuring the flat number and concluding the flank does not matter is the same class of
//! error as reading a blurred mask and concluding the terrain got flatter.
//!
//! Resolved properly — through the track belt, at real hull yaw — the flank is a genuine skill
//! surface, and these tests hold it there.

use game_core::math::{HullPose, plate_normal};
use game_core::{ArmorFacing, ArmorZone, VehicleKind, resolve_penetration_through_screens};
use glam::Vec3;

/// Does this gun's stock round cross the belt and the side plate, with the hull yawed away?
fn through_the_flank(gun: &str, target: VehicleKind, yaw_deg: f32) -> bool {
    let shell = VehicleKind::PLAYABLE
        .iter()
        .flat_map(|kind| kind.gun_options())
        .find(|module| module.spec.name == gun)
        .unwrap_or_else(|| panic!("{gun} is in the catalog"))
        .spec
        .shell;
    let spec = target.spec();
    let slope = spec.hull.facet(ArmorFacing::HullSide).slope_degrees;
    let normal =
        plate_normal(HullPose::level(yaw_deg.to_radians()), 0.0, ArmorZone::HullSide, 1.0, slope);
    let angle = (-Vec3::new(-1.0, 0.0, 0.0)).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
    resolve_penetration_through_screens(
        &shell,
        &spec.hull,
        &[ArmorZone::RightTrack],
        angle,
        angle,
        100.0,
    )
    .penetrated
}

/// Broadside is death for everyone, and that is historically right: no tank of this era carried a
/// side that stopped a contemporary anti-tank round square on.
#[test]
fn a_flat_flank_is_lethal_to_every_hull_in_the_fleet() {
    for target in VehicleKind::PLAYABLE {
        assert!(
            through_the_flank("85 mm ZiS-S-53", target, 0.0),
            "{target:?} must not shrug off the weakest gun in the game broadside"
        );
    }
}

/// ANGLE IS THE SKILL, and it pays differently depending on what you are angling.
///
/// At 45 degrees of hull yaw the Panther's 75 is already stopped by every 80 mm-plus flank in the
/// fleet, and still goes through the three thin ones. That spread is the whole reason side
/// thickness is a stat rather than a decoration.
#[test]
fn angling_saves_a_thick_flank_and_does_not_save_a_thin_one() {
    for thick in [VehicleKind::T54_1951, VehicleKind::TigerII, VehicleKind::IS3] {
        assert!(
            !through_the_flank("7.5 cm KwK 42 L/70", thick, 45.0),
            "{thick:?} carries 80 mm or more of flank; at 45 degrees the 75 must not cross it"
        );
    }
    for thin in [VehicleKind::PantherII, VehicleKind::Centurion, VehicleKind::T34_85] {
        assert!(
            through_the_flank("7.5 cm KwK 42 L/70", thin, 45.0),
            "{thin:?} carries 60 mm or less; angling alone must not save it from the 75"
        );
    }
}

/// The heavier the gun, the harder you must angle — which is what makes the choice of what to
/// present a decision rather than a habit.
#[test]
fn a_bigger_gun_demands_a_steeper_angle_from_the_same_hull() {
    // The T-54's 80 mm flank stops the 75 at 45 degrees and still loses to the 100 there.
    assert!(!through_the_flank("7.5 cm KwK 42 L/70", VehicleKind::T54_1951, 45.0));
    assert!(through_the_flank("100 mm D-10T", VehicleKind::T54_1951, 45.0));
    // Angle harder and the same hull turns the 100 away too.
    assert!(!through_the_flank("100 mm D-10T", VehicleKind::T54_1951, 65.0));
    // The 20-pounder is the gun angling answers worst: it still crosses at 65 degrees.
    assert!(through_the_flank("84 mm 20-pounder Type A", VehicleKind::T54_1951, 65.0));
}
