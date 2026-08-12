//! The visible stern and the armour stern are ONE pair of planes.
//!
//! The rear used to be one 5-degree plate from deck to belly — an angle typed in the data.rs era
//! with no source behind it, on a tail no test had ever fired at. The armour table (ru-wiki,
//! warbook agree) says 45 mm @ 17 degrees for the upper plate, and the calibrated three-view puts
//! the knuckle — the hull's rearmost line — at h ~= 1.20 with an undercut below it. The rebuild
//! authors the pair once in the blueprint (`armor.hull_rear_knuckle` beside `hull_rear`) and
//! points the metal, the armour volumes and the deck's rear edge at it. This file is the
//! agreement's lock.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_build::t54_description;

fn authored() -> (f32, f32, f32) {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let (knuckle_y, lower_deg) =
        bp.armor.hull_rear_knuckle.expect("the T-54 authors its stern knuckle");
    (bp.armor.hull_rear.0, knuckle_y, lower_deg)
}

/// The armour's two stern planes carry the authored angles and share the authored knuckle line.
#[test]
fn the_armour_stern_folds_at_the_authored_knuckle() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let (upper_deg, knuckle_y, lower_deg) = authored();
    assert!((upper_deg - 17.0).abs() < 0.1, "the armour table says 45 mm @ 17, got {upper_deg}");

    let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;
    let knuckle = Vec3::new(0.0, knuckle_y - cy, -bp.hull.half_len);
    let rear_planes: Vec<_> = volumes
        .hull
        .iter()
        .flat_map(|volume| volume.planes.iter())
        .filter(|p| p.zone == ArmorZone::HullRear)
        .collect();
    assert!(rear_planes.len() >= 2, "a knuckled stern is at least two rear planes");

    let mut saw_upper = false;
    let mut saw_lower = false;
    for plane in rear_planes {
        let n = plane.normal;
        assert!(n.z < 0.0, "a stern plane faces rearward");
        let deg = n.y.abs().atan2(-n.z).to_degrees();
        if n.y > 0.0 {
            assert!(
                (deg - upper_deg).abs() < 0.5,
                "the upper plate plays {deg:.1} against an authored {upper_deg}"
            );
            saw_upper = true;
        } else {
            assert!(
                (deg - lower_deg).abs() < 0.5,
                "the undercut plays {deg:.1} against an authored {lower_deg}"
            );
            saw_lower = true;
        }
        let gap = plane.normal.dot(knuckle) - plane.offset;
        assert!(
            gap.abs() < 0.005,
            "a stern plane misses the shared knuckle by {gap:.3} m — two folds again"
        );
    }
    assert!(saw_upper && saw_lower, "both plates of the pair resolve");
}

/// The METAL folds where the armour folds: the hull's rearmost vertex sits on the knuckle line,
/// and the roofline ends where the 17-degree plate leaves it — not a hand's width short.
#[test]
fn the_visible_stern_is_the_armour_stern() {
    let (upper_deg, knuckle_y, lower_deg) = authored();
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let description = t54_description();

    let hull_parts = ["lower_tub", "upper_hull"];
    let mut rearmost = Vec3::ZERO;
    for part in description.parts.iter().filter(|p| hull_parts.contains(&p.key.name)) {
        for v in part.mesh().vertices() {
            if v.position.z < rearmost.z {
                rearmost = v.position;
            }
        }
    }
    assert!(
        (rearmost.z + bp.hull.half_len).abs() < 0.005,
        "the knuckle is the hull's rearmost line, got z {:.3}",
        rearmost.z
    );
    assert!(
        (rearmost.y - knuckle_y).abs() < 0.02,
        "and it sits at the authored height, got y {:.3} vs {knuckle_y}",
        rearmost.y
    );

    // The deck's rear edge: where the upper plate's rake meets the 1.58 roofline.
    let deck_edge_z =
        -bp.hull.half_len + (bp.hull.deck_y - knuckle_y) * upper_deg.to_radians().tan();
    let mut roof_rearmost = f32::INFINITY;
    for part in description.parts.iter().filter(|p| hull_parts.contains(&p.key.name)) {
        for v in part.mesh().vertices() {
            if (v.position.y - bp.hull.deck_y).abs() < 0.01 && v.position.z < roof_rearmost {
                roof_rearmost = v.position.z;
            }
        }
    }
    assert!(
        (roof_rearmost - deck_edge_z).abs() < 0.01,
        "the roofline ends where the upper plate leaves it: got {roof_rearmost:.3} vs \
         {deck_edge_z:.3}"
    );

    // And the undercut genuinely undercuts: at the sponson step the tub's rear sits forward of
    // the knuckle by the authored rake.
    let tub_rear_z =
        -bp.hull.half_len + (knuckle_y - bp.hull.sponson_y) * lower_deg.to_radians().tan();
    let mut tub_rearmost = f32::INFINITY;
    for part in description.parts.iter().filter(|p| p.key.name == "lower_tub") {
        for v in part.mesh().vertices() {
            if v.position.z < tub_rearmost {
                tub_rearmost = v.position.z;
            }
        }
    }
    assert!(
        (tub_rearmost - tub_rear_z).abs() < 0.01,
        "the tub's rear rides the undercut: got {tub_rearmost:.3} vs {tub_rear_z:.3}"
    );
}
