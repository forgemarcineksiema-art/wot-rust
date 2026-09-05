//! The fleet part library, hull: tub, upper hull, engine-deck panels and the louvred grille as
//! convex solids. Moved out of the `solid` KERNEL (Forge 2.0 K2): a kernel knows planes, a part
//! library knows what a hull is. Every dimension is read from the blueprint's [`HullVisual`] /
//! [`BoxVisual`] / [`DetailVisual`]; the plate *slopes* are passed in from the blueprint's armour
//! facets, so the visible rake of each facet is the same angle the penetration model uses — what
//! you see is what you shoot. Grown on the T-54; any vehicle carrying these visuals builds here.

use game_core::{BoxVisual, DetailVisual, HullPlatesVisual, HullShape, HullVisual};
use glam::Vec3;
use solid::chamfer;
use solid::chamfered_box;

use solid::{ConvexSolid, Plane};

/// The stern's plane pair. With no knuckle the rear is the fleet's single plate: the upper-plate
/// plane anchored so the hull's rearmost point sits at `anchor_y` (the caller's bottom edge), and
/// no second plane. With a knuckle `(knuckle_y, lower_deg)` the rear becomes the T-54's overhung
/// tail: the upper plate's rake anchors THROUGH the knuckle line at `(knuckle_y, -half_len)` — the
/// rearmost line of the hull — and an undercut plane leans forward-down below it. Both planes pass
/// through the knuckle, so the two metal faces and the two armour faces share one fold, exactly as
/// the bow's glacis/nose pair does.
fn stern_planes(
    half_len: f32,
    rear_deg: f32,
    knuckle: Option<(f32, f32)>,
    anchor_y: f32,
) -> (Plane, Option<Plane>) {
    let rear = rear_deg.to_radians();
    match knuckle {
        None => {
            let rear_off = rear.sin() * anchor_y + rear.cos() * half_len;
            (Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), rear_off), None)
        }
        Some((knuckle_y, lower_deg)) => {
            let upper_off = rear.sin() * knuckle_y + rear.cos() * half_len;
            let lower = lower_deg.to_radians();
            // Outward normal rear-and-DOWN: below the knuckle the plate tucks forward.
            let lower_off = -lower.sin() * knuckle_y + lower.cos() * half_len;
            (
                Plane::new(Vec3::new(0.0, rear.sin(), -rear.cos()), upper_off),
                Some(Plane::new(Vec3::new(0.0, -lower.sin(), -lower.cos()), lower_off)),
            )
        }
    }
}

/// The wide upper hull (sponson body): a convex solid from the sponson step up to the deck, whose
/// front is the steep upper glacis, sides rake inward, and rear rakes up — each at its blueprint
/// armour angle. It overhangs the narrower lower tub, forming the sponson step over the tracks.
/// An authored stern knuckle folds the rear into its upper-plate/undercut pair.
pub fn upper_hull_solid(
    h: &HullShape,
    p: &HullPlatesVisual,
    front_deg: f32,
    side_deg: f32,
    rear_deg: f32,
    knuckle: Option<(f32, f32)>,
) -> ConvexSolid {
    let (front, side) = (front_deg.to_radians(), side_deg.to_radians());
    let side_off = h.half_width * side.cos() + h.sponson_y * side.sin();
    let glacis_off = front.sin() * h.sponson_y + front.cos() * p.glacis_base_z;
    let (rear_upper, rear_lower) = stern_planes(h.half_len, rear_deg, knuckle, h.sponson_y);
    // The T-54's narrow box carries ONE plane per facet — the glacis is a plain full-hull-width
    // plate with no bow taper (the exposed tracks flank it; the plan stays rectangular).
    let mut planes = vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -h.sponson_y),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), h.deck_y),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), glacis_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        rear_upper,
    ];
    planes.extend(rear_lower);
    ConvexSolid::new(planes)
}

/// The narrow lower tub between the tracks: a convex solid from the belly to the sponson step, with
/// vertical sides at the tub half-width, a raked rear (the stern undercut, where authored), and a
/// lower nose plate that folds under the upper glacis at the blueprint fold line.
pub fn lower_tub_solid(
    h: &HullShape,
    p: &HullPlatesVisual,
    rear_deg: f32,
    knuckle: Option<(f32, f32)>,
) -> ConvexSolid {
    // The tub sits entirely below the knuckle, so only ONE of the stern pair can bind here: the
    // undercut where authored, the single fleet plate otherwise.
    let (rear_single, rear_undercut) = stern_planes(h.half_len, rear_deg, knuckle, h.belly_y);
    let rear_plane = rear_undercut.unwrap_or(rear_single);
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
        rear_plane,
        Plane::new(nose_normal, nose_off),
    ])
}

/// The full hull as a convex solid: a block whose glacis, sloped sides and sloped rear carry the
/// blueprint armour angles (degrees of the plate normal above horizontal), plus a lower nose bevel
/// and — where authored — the stern's knuckle/undercut pair.
pub fn hull_solid(
    hull: &HullVisual,
    front_deg: f32,
    side_deg: f32,
    rear_deg: f32,
    knuckle: Option<(f32, f32)>,
) -> ConvexSolid {
    let (front, side) = (front_deg.to_radians(), side_deg.to_radians());
    let (hx, belly, roof, hz) = (hull.half_width, hull.belly_y, hull.roof_y, hull.half_len);
    let side_off = hx * side.cos() + belly * side.sin();
    let mut planes = vec![
        Plane::new(Vec3::new(0.0, -1.0, 0.0), -belly),
        Plane::new(Vec3::new(0.0, 1.0, 0.0), roof),
        Plane::new(Vec3::new(side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(-side.cos(), side.sin(), 0.0), side_off),
        Plane::new(Vec3::new(0.0, front.sin(), front.cos()), hull.glacis_offset),
        Plane::new(hull.nose_normal, hull.nose_offset),
    ];
    let (rear_upper, rear_lower) = stern_planes(hz, rear_deg, knuckle, belly);
    planes.push(rear_upper);
    planes.extend(rear_lower);
    ConvexSolid::new(planes)
}

/// The engine deck split into three recognizable panels (front/centre/rear) separated by thin seam
/// gaps, so the rear deck reads as bolted plates rather than one slab. Flat tops, hard edges — the
/// CAD generator keeps the plate normals crisp. The split is visual only; the deck footprint is
/// unchanged. Clean factory build: panel seams, not weld scars or weathering.
pub fn engine_deck_panels(deck: &BoxVisual) -> Vec<ConvexSolid> {
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
pub fn deck_grille(d: &DetailVisual, deck_top: f32) -> Vec<ConvexSolid> {
    let (c, h) = (d.grille_center, d.grille_half);
    let rail = 0.04_f32;
    let depth = 0.12_f32;
    let mut solids = vec![
        // The shadowed well under the louvers.
        ConvexSolid::box_at(
            Vec3::new(c.x, deck_top + 0.002 - depth * 0.5, c.z),
            Vec3::new(h.x - rail, depth * 0.5, h.z - rail),
        ),
        // THE BEVEL LAW, applied the way this repo already knew how. A flame-cut frame rail has an
        // arris a viewer reads as the light on the edge of steel; a perfectly sharp one has no
        // face turned to the sky at all, and that missing highlight is the most reliable tell
        // that a shape came out of a computer. `chamfer::FLAME_CUT` is what a torch leaves.
        chamfered_box(
            Vec3::new(c.x, c.y, c.z + h.z),
            Vec3::new(h.x, h.y, rail),
            chamfer::FLAME_CUT,
        ),
        chamfered_box(
            Vec3::new(c.x, c.y, c.z - h.z),
            Vec3::new(h.x, h.y, rail),
            chamfer::FLAME_CUT,
        ),
        chamfered_box(
            Vec3::new(c.x + h.x, c.y, c.z),
            Vec3::new(rail, h.y, h.z),
            chamfer::FLAME_CUT,
        ),
        chamfered_box(
            Vec3::new(c.x - h.x, c.y, c.z),
            Vec3::new(rail, h.y, h.z),
            chamfer::FLAME_CUT,
        ),
    ];
    let slats = d.grille_slats.max(1);
    let slat_half_z = (h.z - rail) / (slats as f32 * 2.0);
    for slat in 0..slats {
        let t = (slat as f32 + 0.5) / slats as f32;
        let z = c.z - h.z + rail + t * 2.0 * (h.z - rail);
        solids.push(raked_slat(
            Vec3::new(c.x, c.y + h.y * 0.4, z),
            h.x - rail,
            h.y * 0.6,
            slat_half_z,
            LOUVRE_RAKE_RAD,
        ));
    }
    solids
}

/// How far a deck louvre leans back from vertical. A louvre that does not lean is a plank: the
/// slats here were axis-aligned boxes, so the "louvered" grille was a row of flat boards over a
/// hole and the shadow band under them did all the work.
const LOUVRE_RAKE_RAD: f32 = 0.62;

/// One louvre plate: a box rotated about X, built from its own six planes because a raked slat is
/// not an axis-aligned box and `box_at` can only make one of those.
fn raked_slat(centre: Vec3, half_x: f32, half_up: f32, half_thick: f32, rake: f32) -> ConvexSolid {
    let (sin, cos) = rake.sin_cos();
    // The slat's own frame: it still spans X, but its width and thickness axes are tilted in YZ.
    let up = Vec3::new(0.0, cos, sin);
    let thick = Vec3::new(0.0, -sin, cos);
    let mut planes = Vec::with_capacity(6);
    for (axis, half) in [(Vec3::X, half_x), (up, half_up), (thick, half_thick)] {
        planes.push(Plane::new(axis, axis.dot(centre) + half));
        planes.push(Plane::new(-axis, (-axis).dot(centre) + half));
    }
    ConvexSolid::new(planes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn hull() -> HullVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .complete_visual()
            .unwrap()
            .hull
            .to_owned()
    }

    #[test]
    fn the_hull_solid_carries_the_armour_glacis_angle() {
        let bp =
            game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951).unwrap();
        let mesh = hull_solid(
            &hull(),
            bp.armor.hull_front.0,
            bp.armor.hull_side.0,
            bp.armor.hull_rear.0,
            bp.armor.hull_rear_knuckle,
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
