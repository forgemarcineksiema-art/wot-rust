//! The ricochet is the END of a slope, not a cliff.
//!
//! Line-of-sight thickness already prices obliquity geometrically: the same plate is longer
//! through at an angle. What it never priced is the shell TURNING — at a shallow enough angle the
//! nose skids across the face instead of biting into it. So the model had 69.9° as a shot at full
//! penetration and 70.1° as a clean bounce: a tenth of a degree between everything and nothing,
//! which no player can read and no gunner ever experienced.

use game_core::{ArmorFacing, ShellSpec, VehicleKind, resolve_penetration_at_distance};

/// A real plate thin enough that the ANGLE decides rather than the steel: the T-34-85's 45 mm
/// rear, against the 85 mm gun that tank carries.
fn shot(angle_degrees: f32) -> game_core::PenetrationResult {
    resolve_penetration_at_distance(
        &ap(),
        &VehicleKind::T34_85.spec().hull,
        ArmorFacing::HullRear,
        angle_degrees,
        100.0,
    )
}

fn ap() -> ShellSpec {
    ShellSpec::armor_piercing(85.0, 792.0, 145.0, 200)
}

#[test]
fn a_square_hit_loses_nothing_to_glancing() {
    for angle in [0.0_f32, 30.0, 55.0, 60.0] {
        let r = shot(angle);
        assert_eq!(r.glance_loss, 0.0, "at {angle}° the round is still biting, not skidding");
    }
}

#[test]
fn the_band_ramps_and_reaches_its_full_cost_at_the_bounce_angle() {
    let mut previous = 0.0;
    for angle in [61.0_f32, 63.0, 65.0, 67.0, 69.0] {
        let loss = shot(angle).glance_loss;
        assert!(loss > previous, "the cost must rise with the angle: {angle}° gave {loss}");
        previous = loss;
    }
    let at_bounce = shot(70.0).glance_loss;
    assert!(
        (at_bounce - 0.30).abs() < 1.0e-4,
        "a round arriving at the bounce angle has lost a third of its bite, got {at_bounce}"
    );
}

/// The point of the band: a shot that used to be a full-strength hit at 69° is now a weak one, so
/// the bounce at 70° is the arrival of something the shell had been losing for ten degrees.
#[test]
fn a_near_glance_is_a_weak_hit_rather_than_a_coin_flip() {
    let square = shot(0.0);
    let near_glance = shot(69.0);
    assert!(square.penetrated, "square on, a 145 mm round opens a 45 mm plate");
    assert!(
        near_glance.remaining_penetration_mm < square.remaining_penetration_mm,
        "the near-glance must arrive with less left than the square hit"
    );
    assert!(!near_glance.ricocheted, "69° is inside the band, not past it");
    assert!(shot(71.0).ricocheted, "and 71° is past it");
}

/// The blunt-nose identity (Amunicja 3.0 B5): the Soviet APBC was BUILT for sloped plate. With
/// the same penetration column, the blunt round turns more of the slope into the armor (8° vs
/// 5°) and digs in three degrees past the sharp nose's bounce — so there is an angle where the
/// Pzgr-style round skids and the BR-style round opens the plate. Concrete shells now fight
/// differently, not just carry different numbers.
#[test]
fn the_blunt_nose_fights_the_slope_where_the_sharp_one_skids() {
    let sharp = ap();
    let blunt = ap().with_penetrator(game_core::Penetrator::FullBoreBlunt);
    let hull = VehicleKind::T34_85.spec().hull;
    let at = |shell: &ShellSpec, angle: f32| {
        resolve_penetration_at_distance(shell, &hull, ArmorFacing::HullRear, angle, 100.0)
    };

    // 71.5° sits between the two bounce angles: the sharp nose skids, the blunt one bites in
    // and — with its deeper normalization — still opens the 45 mm plate.
    let sharp_shot = at(&sharp, 71.5);
    let blunt_shot = at(&blunt, 71.5);
    assert!(sharp_shot.ricocheted, "past 70° the sharp nose is gone");
    assert!(!blunt_shot.ricocheted, "the blunt nose is still digging at 71.5°");
    assert!(blunt_shot.penetrated, "and it opens the plate: {blunt_shot:?}");

    // Inside the band both bite, but the blunt round arrives with more left: it turned more of
    // the slope away before the LOS steel was measured.
    let sharp_oblique = at(&sharp, 65.0);
    let blunt_oblique = at(&blunt, 65.0);
    assert!(
        blunt_oblique.remaining_penetration_mm > sharp_oblique.remaining_penetration_mm,
        "same column, different noses: blunt {} vs sharp {} mm left",
        blunt_oblique.remaining_penetration_mm,
        sharp_oblique.remaining_penetration_mm
    );
    // And square on there is nothing to normalize: the noses are equals.
    assert_eq!(
        at(&sharp, 0.0).remaining_penetration_mm,
        at(&blunt, 0.0).remaining_penetration_mm,
        "the identity is about SLOPE, not a flat bonus"
    );
}

/// A shaped charge does not bite, so it does not skid: its obliquity limit is its own.
#[test]
fn heat_and_high_explosive_are_untouched_by_the_band() {
    let heat = ShellSpec::heat(100.0, 900.0, 280.0, 320);
    let he = ShellSpec::high_explosive(100.0, 900.0, 33.0, 430, 2.0);
    let hull = VehicleKind::T34_85.spec().hull;
    for angle in [61.0_f32, 65.0, 69.0] {
        for shell in [&heat, &he] {
            let r =
                resolve_penetration_at_distance(shell, &hull, ArmorFacing::HullRear, angle, 100.0);
            assert_eq!(r.glance_loss, 0.0, "{:?} does not skid", shell.shell_type);
        }
    }
}
