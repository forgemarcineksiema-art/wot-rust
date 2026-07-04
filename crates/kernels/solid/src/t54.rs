//! The T-54 hull and its appendage plates as convex solids — the CAD/B-rep arm of the hybrid kernel.
//!
//! Every dimension is read from the vehicle blueprint's [`HullVisual`] / [`BoxVisual`] /
//! [`FenderVisual`]; the plate *slopes* are passed in from the blueprint's armour facets, so the
//! visible rake of each facet is the same angle the penetration model uses — what you see is what
//! you shoot.

use game_core::{BoxVisual, DetailVisual, HullPlatesVisual, HullShape, HullVisual};
use glam::Vec3;

use crate::{ConvexSolid, Plane};

/// The wide upper hull (sponson body): a convex solid from the sponson step up to the deck, whose
/// front is the steep upper glacis, sides rake inward, and rear rakes up — each at its blueprint
/// armour angle. It overhangs the narrower lower tub, forming the sponson step over the tracks.
pub fn t54_upper_hull(
    h: &HullShape,
    p: &HullPlatesVisual,
    front_deg: f32,
    side_deg: f32,
    rear_deg: f32,
) -> ConvexSolid {
    let (front, side, rear) =
        (front_deg.to_radians(), side_deg.to_radians(), rear_deg.to_radians());
    let side_off = h.half_width * side.cos() + h.sponson_y * side.sin();
    let glacis_off = front.sin() * h.sponson_y + front.cos() * p.glacis_base_z;
    let rear_off = rear.sin() * h.sponson_y + rear.cos() * h.half_len;
    // The T-54's narrow box carries ONE plane per facet — the glacis is a plain full-hull-width
    // plate with no bow taper (the exposed tracks flank it; the plan stays rectangular).
    ConvexSolid::new(vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -h.sponson_y),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), h.deck_y),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), rear_off),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), glacis_off),
    ])
}

/// The narrow lower tub between the tracks: a convex solid from the belly to the sponson step, with
/// vertical sides at the tub half-width, a raked rear, and a lower nose plate that folds under the
/// upper glacis at the blueprint fold line.
pub fn t54_lower_tub(h: &HullShape, p: &HullPlatesVisual, rear_deg: f32) -> ConvexSolid {
    let rear = rear_deg.to_radians();
    let rear_off = rear.sin() * h.belly_y + rear.cos() * h.half_len;
    // Lower nose plate: the plane through the fold line (sponson step) and the tucked-back belly
    // front, spanning the tub width. Its forward-and-down normal carries the lower-glacis rake.
    let dz = p.nose_base_z - p.glacis_base_z;
    let dy = h.belly_y - h.sponson_y;
    let nose_normal = Vec3::new(0.0, -dz / dy, 1.0);
    let nose_off = nose_normal.dot(Vec3::new(0.0, h.sponson_y, p.glacis_base_z));
    ConvexSolid::new(vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -h.belly_y),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), h.sponson_y),
        Plane::new(Vec3::new(1.0, 0.0, 0.0), h.lower_half_width),
        Plane::new(Vec3::new(-1.0, 0.0, 0.0), h.lower_half_width),
        Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), rear_off),
        Plane::new(nose_normal, nose_off),
    ])
}

/// The full hull as a convex solid: a block whose glacis, sloped sides and sloped rear carry the
/// blueprint armour angles (degrees of the plate normal above horizontal), plus a lower nose bevel.
pub fn t54_hull_solid(
    hull: &HullVisual,
    front_deg: f32,
    side_deg: f32,
    rear_deg: f32,
) -> ConvexSolid {
    let (front, side, rear) =
        (front_deg.to_radians(), side_deg.to_radians(), rear_deg.to_radians());
    let (hx, belly, roof, hz) = (hull.half_width, hull.belly_y, hull.roof_y, hull.half_len);
    let side_off = hx * side.cos() + belly * side.sin();
    ConvexSolid::new(vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -belly),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), roof),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), belly * rear.sin() + hz * rear.cos()),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), hull.glacis_offset),
        Plane::new(hull.nose_normal, hull.nose_offset),
    ])
}

/// The hull front / glacis as a two-plate solid: the hull block clipped by the upper glacis plate
/// (normal `slope` degrees above horizontal) and the lower nose bevel. The CAD counterpart of the
/// SDF glacis in `sdf_mesh`, built from the same dimensions for a like-for-like sharpness comparison.
pub fn t54_glacis_solid(hull: &HullVisual, slope_deg: f32) -> ConvexSolid {
    let slope = slope_deg.to_radians();
    let center = Vec3::new(0.0, 0.5 * (hull.belly_y + hull.roof_y), 0.0);
    let half = Vec3::new(hull.half_width, 0.5 * (hull.roof_y - hull.belly_y), hull.half_len);
    let glacis = Plane::new(Vec3::new(0.0, slope.sin(), slope.cos()), hull.glacis_offset);
    let nose = Plane::new(hull.nose_normal, hull.nose_offset);
    ConvexSolid::box_at(center, half).clipped_by(glacis).clipped_by(nose)
}

/// The raised rear engine deck panel behind the turret.
pub fn t54_engine_deck(deck: &BoxVisual) -> ConvexSolid {
    ConvexSolid::box_at(deck.center, deck.half)
}

/// The engine deck split into three recognizable panels (front/centre/rear) separated by thin seam
/// gaps, so the rear deck reads as bolted plates rather than one slab. Flat tops, hard edges — the
/// CAD generator keeps the plate normals crisp. The split is visual only; the deck footprint is
/// unchanged. Clean factory build: panel seams, not weld scars or weathering.
pub fn t54_engine_deck_panels(deck: &BoxVisual) -> Vec<ConvexSolid> {
    let panel_half_z = deck.half.z / 3.0 - 0.02;
    let half = Vec3::new(deck.half.x - 0.02, deck.half.y, panel_half_z);
    (-1..=1)
        .map(|row| {
            let center = Vec3::new(
                deck.center.x,
                deck.center.y,
                deck.center.z + row as f32 * deck.half.z / 1.5,
            );
            ConvexSolid::box_at(center, half)
        })
        .collect()
}

/// The louvered engine-deck grille: a raised frame around evenly spaced slats, over a shallow well
/// (top a hair proud of `deck_top`) that the "engine_grille" surface bake drops into shadow — so the
/// slat gaps read as a dark cooling intake, not the bright deck plate. All boxes, all hard-edged.
pub fn t54_deck_grille(d: &DetailVisual, deck_top: f32) -> Vec<ConvexSolid> {
    let (c, h) = (d.grille_center, d.grille_half);
    let rail = 0.04_f32;
    let depth = 0.12_f32;
    let mut solids = vec![
        // The shadowed well under the louvers.
        ConvexSolid::box_at(
            Vec3::new(c.x, deck_top + 0.002 - depth * 0.5, c.z),
            Vec3::new(h.x - rail, depth * 0.5, h.z - rail),
        ),
        ConvexSolid::box_at(Vec3::new(c.x, c.y, c.z + h.z), Vec3::new(h.x, h.y, rail)),
        ConvexSolid::box_at(Vec3::new(c.x, c.y, c.z - h.z), Vec3::new(h.x, h.y, rail)),
        ConvexSolid::box_at(Vec3::new(c.x + h.x, c.y, c.z), Vec3::new(rail, h.y, h.z)),
        ConvexSolid::box_at(Vec3::new(c.x - h.x, c.y, c.z), Vec3::new(rail, h.y, h.z)),
    ];
    let slats = d.grille_slats.max(1);
    let slat_half_z = (h.z - rail) / (slats as f32 * 2.0);
    for slat in 0..slats {
        let t = (slat as f32 + 0.5) / slats as f32;
        let z = c.z - h.z + rail + t * 2.0 * (h.z - rail);
        solids.push(ConvexSolid::box_at(
            Vec3::new(c.x, c.y + h.y * 0.4, z),
            Vec3::new(h.x - rail, h.y * 0.6, slat_half_z),
        ));
    }
    solids
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn hull() -> HullVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .hybrid()
            .unwrap()
            .hull
    }

    #[test]
    fn the_glacis_solid_meshes_to_a_handful_of_crisp_triangles() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let mesh = t54_glacis_solid(&hull(), bp.armor.hull_front.0)
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("glacis solid is valid");
        assert!(mesh.triangle_count() > 0, "glacis solid is empty");
        assert!(
            mesh.triangle_count() < 40,
            "exact convex glacis stays tiny: {}",
            mesh.triangle_count()
        );
    }

    #[test]
    fn the_hull_solid_carries_the_armour_glacis_angle() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let mesh = t54_hull_solid(
            &hull(),
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
        )
        .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
        .expect("hull solid is valid");
        let on_slope = mesh.vertices().iter().any(|v| {
            let n = v.normal;
            n.y > 0.2
                && n.z > 0.2
                && (n.y.atan2(n.z).to_degrees() - bp.armor.hull_front.0).abs() < 2.0
        });
        assert!(on_slope, "a glacis face normal carries the blueprint armour slope");
    }
}
