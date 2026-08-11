//! The visible nose and the armour nose are ONE plane.
//!
//! The lower front plate used to be three different plates wearing one name: the metal folded at
//! 36.8 degrees (an authored setback constant), the armour volume played 27 (the fleet's
//! 0.45-of-the-glacis derivation), and the dossier says 55 — with the armour's fold anchored
//! 50 mm ahead of the visible one for good measure. A shell meeting the bow met none of what the
//! player saw. The cascade audit (2026-08-11) mapped it; the fix authors the angle once in the
//! blueprint and points every consumer at it. This file is the agreement's lock.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;

/// The authored plate, the visible plate and the armour plate carry one angle and one fold.
#[test]
fn the_visible_nose_and_the_armour_nose_are_one_plane() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let (authored_deg, _) = bp.armor.hull_lower_front.expect("the T-54 authors its lower plate");
    assert!(
        (authored_deg - 55.0).abs() < 0.1,
        "the dossier says 100 mm @ 55 deg; the blueprint authors {authored_deg}"
    );

    // The VISIBLE plate: the hybrid's nose plane, as the mesh generators read it.
    let visual = bp.complete_visual().expect("hybrid visual");
    let nose = visual.hull.nose_normal.normalize();
    // The armour convention: a plate's slope is measured from vertical, and the nose's outward
    // normal tips forward-and-down, so the angle from vertical is atan2(|n.y|, n.z).
    let visual_deg = nose.y.abs().atan2(nose.z).to_degrees();
    assert!(
        (visual_deg - authored_deg).abs() < 0.5,
        "the metal folds at {visual_deg:.1} deg against an authored {authored_deg} — the nose \
         setback stopped reading the blueprint"
    );

    // The ARMOUR plate: the volume's LowerPlate plane.
    let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;
    let plate = volumes
        .hull
        .iter()
        .flat_map(|volume| volume.planes.iter())
        .find(|p| p.zone == ArmorZone::LowerPlate && p.normal.z > 0.3)
        .expect("the hull volume carries a lower front plate");
    let armour_deg = plate.normal.y.abs().atan2(plate.normal.z).to_degrees();
    assert!(
        (armour_deg - authored_deg).abs() < 0.5,
        "the armour plays {armour_deg:.1} deg against an authored {authored_deg} — the volume \
         went back to deriving the plate from the glacis"
    );

    // And ONE fold: the armour plane passes through the visible fold line, not 50 mm ahead of
    // it. The fold is where the glacis meets the nose, at the sponson step height.
    let fold = Vec3::new(0.0, bp.hull.sponson_y - cy, visual.hull_plates.glacis_base_z);
    let gap = plate.normal.dot(fold) - plate.offset;
    assert!(
        gap.abs() < 0.005,
        "the armour's nose plane misses the visible fold by {gap:.3} m — two folds again"
    );
}
