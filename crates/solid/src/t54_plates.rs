//! Visible hull-plate articulation for the T-54: the weld seams that mark the main plate joins and
//! the bolt-on rear transmission covers. These are visual-only detail solids sitting on top of the
//! structural hull (which stays a clean CAD body); none adds an armour, collision or mount dimension.

use game_core::{BoxVisual, HullVisual};
use glam::Vec3;

use crate::ConvexSolid;

/// Weld seams articulating the main hull plates: a transverse bead where the upper glacis meets the
/// roof. The join is recovered from the hull dimensions and the glacis slope (the single armour
/// source) — the glacis plane `sin*y + cos*z = glacis_offset` intersected with the roof `y = roof_y`
/// — so the bead sits exactly on the real plate join, not a guessed line.
pub fn t54_hull_plate_seams(hull: &HullVisual, front_deg: f32) -> Vec<ConvexSolid> {
    let front = front_deg.to_radians();
    let z = (hull.glacis_offset - front.sin() * hull.roof_y) / front.cos();
    let bead = 0.02_f32;
    vec![ConvexSolid::box_at(
        Vec3::new(0.0, hull.roof_y, z),
        Vec3::new(hull.half_width * 0.6, bead, bead),
    )]
}

/// The bolt-on rear transmission access covers: two raised plates flanking the central engine-deck
/// grille — the deck's transmission/engine access hatches. Derived from the deck box, raised proud of
/// its top and kept clear of the grille so they read as separate covers.
pub fn t54_transmission_covers(deck: &BoxVisual) -> Vec<ConvexSolid> {
    let top = deck.center.y + deck.half.y;
    let half = Vec3::new(0.16, 0.03, deck.half.z * 0.72);
    [1.0_f32, -1.0]
        .map(|side| {
            let center = Vec3::new(side * (deck.half.x - 0.20), top, deck.center.z);
            ConvexSolid::box_at(center, half)
        })
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn visual() -> game_core::HybridVisual {
        *game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
    }

    #[test]
    fn the_glacis_roof_seam_sits_on_the_plate_join() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let v = visual();
        let seams = t54_hull_plate_seams(&v.hull, bp.armor.hull_front.0);
        assert!(!seams.is_empty(), "the hull carries a glacis-to-roof weld seam");
        let b = seams[0]
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .bounds()
            .expect("non-empty seam");
        // The seam straddles the roof height and lies just ahead of the turret ring, on the join.
        let mid_y = 0.5 * (b.min.y + b.max.y);
        assert!((mid_y - v.hull.roof_y).abs() < 0.05, "seam straddles the roof plane");
        assert!(b.min.z > 1.0, "seam sits forward on the upper glacis join");
    }

    #[test]
    fn the_transmission_covers_are_raised_plates_flanking_the_deck() {
        let v = visual();
        let covers = t54_transmission_covers(&v.deck);
        assert_eq!(covers.len(), 2, "two flanking transmission covers");
        let deck_top = v.deck.center.y + v.deck.half.y;
        let mut sides = (false, false);
        for cover in &covers {
            let b = cover
                .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
                .bounds()
                .expect("non-empty cover");
            assert!(b.max.y > deck_top + 0.02, "cover stands proud of the deck top");
            let cx = 0.5 * (b.min.x + b.max.x);
            if cx > 0.0 {
                sides.0 = true;
            } else {
                sides.1 = true;
            }
        }
        assert!(sides.0 && sides.1, "a cover on each side of the deck centreline");
    }
}
