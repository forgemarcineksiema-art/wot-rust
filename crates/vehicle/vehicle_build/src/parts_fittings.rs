//! The fleet part library, fittings: fender furniture and small pressed fittings as convex solids.
//! Moved out of the `solid` kernel (Forge 2.0 K2). Every dimension is read from the blueprint's
//! [`FenderVisual`] / [`DetailVisual`]; nothing here invents a tank dimension.

use game_core::{DetailVisual, FenderVisual};
use glam::{Vec2, Vec3};

use solid::{ConvexSolid, Plane, chamfered_box};

/// One sloping fender end section (front mudguard dropping over the idler, or the rear flap over
/// the sprocket): a thin plate falling from the flat fender run down toward the track end, as the
/// references show. `sign` is +1 for the front section, -1 for the rear.
/// The pressed stiffening ribs on a hanging tail flap: three raised beads down the flap's
/// face, as the reference rear view shows. The flap itself is the mudguard sweep's last
/// segment (`vehicle_build::t54_kit`); the ribs ride that segment's plane, a bead proud.
pub fn flap_ribs(side_x: f32, top: Vec2, bottom: Vec2, half_x: f32) -> Vec<ConvexSolid> {
    let top3 = Vec3::new(side_x, top.y, top.x);
    let bottom3 = Vec3::new(side_x, bottom.y, bottom.x);
    let along = (bottom3 - top3).normalize();
    let normal_raw = Vec3::X.cross(along).normalize();
    // Outward: away from the hull centre plane in z (the flap faces the end of the tank).
    let normal = if normal_raw.z * top.x.signum() >= 0.0 { normal_raw } else { -normal_raw };
    let mid = 0.5 * (top3 + bottom3);
    let centre = mid + normal * (0.005 + 0.010);
    let half_run = (bottom3 - top3).length() * 0.5 - 0.015;
    (-1..=1)
        .map(|k| {
            let c = centre + Vec3::X * (k as f32 * (half_x * 0.6));
            let mut planes = Vec::with_capacity(6);
            for (axis, half) in [(Vec3::X, 0.016), (along, half_run), (normal, 0.010)] {
                planes.push(Plane::new(axis, axis.dot(c) + half));
                planes.push(Plane::new(-axis, (-axis).dot(c) + half));
            }
            ConvexSolid::new(planes)
        })
        .collect()
}

/// Thin gusset plates under the fender's INNER edge, evenly spaced along its run — the supports
/// that carry the shelf, triangulating against the hull wall the way real fender gussets do.
/// With the shelf at its corrected 1.35 sheet plane (2026-08-12) the gussets hang in the open
/// daylight band over the crest links, far clear of anything a jounced wheel sweeps. Visual
/// only, close-up detail tier.
pub fn fender_brackets(side_x: f32, fender: &FenderVisual) -> Vec<ConvexSolid> {
    const BRACKETS: usize = 5;
    let drop = 0.04_f32;
    let bottom = fender.center_y - fender.half.y;
    // Inboard: from the shelf's inner edge out over the first sliver of the belt band only.
    let inner_x = side_x - side_x.signum() * (fender.half.x - 0.07);
    let half = Vec3::new(0.07, drop * 0.5, 0.012);
    (0..BRACKETS)
        .map(|i| {
            let t = (i as f32 + 0.5) / BRACKETS as f32;
            let z = -fender.half.z + t * 2.0 * fender.half.z;
            ConvexSolid::box_at(Vec3::new(inner_x, bottom - drop * 0.5, z), half)
        })
        .collect()
}

/// The louvered exhaust box on the left fender at the engine bay — a chamfered armoured housing,
/// the clean factory exhaust, not a sooted pipe.
pub fn exhaust_housing(d: &DetailVisual) -> ConvexSolid {
    chamfered_box(d.exhaust_center, d.exhaust_half, 0.03)
}

/// A periscope head: a small housing box whose front-top edge is sliced by a forward-and-up plane,
/// giving the raked prism (viewing-glass) face that reads as a real periscope rather than a plain
/// block. The slant looks forward (+z); mirror the centre in x for the opposite-hand device.
pub fn periscope(center: Vec3, half: Vec3) -> ConvexSolid {
    // A 45-degree chamfer of depth `s` off the front-top edge: the prism glass rakes back as it
    // rises. `s` is a fraction of the head so the cut never crosses the box for any sane periscope.
    let s = 0.7 * half.y.min(half.z);
    let normal = Vec3::new(0.0, 1.0, 1.0);
    let through = Vec3::new(center.x, center.y + half.y - s, center.z + half.z);
    ConvexSolid::box_at(center, half).clipped_by(Plane::new(normal, normal.dot(through)))
}

/// The PRISM a periscope looks through: a thin glass slab lying in the head's raked face, inset
/// from its edges so the housing frames it, standing a hair proud so the two never z-fight.
///
/// The head alone is a box with one corner cut — seven planes — and that is what both the driver's
/// and the turret's periscopes were. A vision device is a housing, a guard and a prism face; the
/// glass is the part a crewman actually uses and the only part that catches the sun.
pub fn periscope_prism(center: Vec3, half: Vec3) -> ConvexSolid {
    let cut = 0.7 * half.y.min(half.z);
    let normal = Vec3::new(0.0, 1.0, 1.0).normalize();
    let face = Vec3::new(center.x, center.y + half.y - cut, center.z + half.z);
    // A hair proud of the housing's rake, so the glass reads as glass rather than as z-fighting.
    let outer = normal.dot(face) + 0.0015;
    let thickness = (cut * 0.5).min(0.012);
    ConvexSolid::box_at(center, half * 0.82)
        .clipped_by(Plane::new(normal, outer))
        .clipped_by(Plane::new(-normal, -(outer - thickness)))
}

/// The armoured cheeks either side of a periscope head. A prism standing bare on a roof is a
/// prism nobody kept; the cheeks are why the crew still has one after the first burst.
pub fn periscope_guards(center: Vec3, half: Vec3) -> [ConvexSolid; 2] {
    let thickness = (half.x * 0.22).max(0.006);
    [-1.0_f32, 1.0].map(|side| {
        chamfered_box(
            Vec3::new(
                center.x + side * (half.x + thickness),
                center.y + half.y * 0.20,
                center.z + half.z * 0.10,
            ),
            Vec3::new(thickness, half.y * 0.90, half.z * 0.78),
            thickness * 0.4,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn fender() -> FenderVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .complete_visual()
            .unwrap()
            .fender
            .to_owned()
    }

    /// REWRITTEN 2026-08-12 (twice): first with the shelf mis-parked on the crest, now with the
    /// shelf at its corrected 1.35 sheet plane. The gussets triangulate at the hull wall just
    /// under the sheet, hanging in the daylight band the reference shows over the crest links —
    /// they must never reach back down toward the run a jounced wheel sweeps.
    #[test]
    fn fender_brackets_hug_the_hull_side_below_the_shelf() {
        let f = fender();
        let brackets = fender_brackets(1.32, &f);
        assert!(brackets.len() >= 3, "several brackets along the run");
        let bottom = f.center_y - f.half.y;
        for bracket in &brackets {
            let b = bracket
                .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
                .expect("bracket is valid")
                .bounds()
                .expect("non-empty bracket");
            assert!(b.min.y < bottom - 0.02, "bracket hangs below the fender plate");
            // The crest links top out at ~0.934 (placed-link measurement); the gussets live a
            // third of a metre above them, just under the corrected sheet.
            assert!(b.min.y > 1.25, "bracket stays up in the daylight band under the sheet");
            assert!(
                b.max.x < 1.25,
                "bracket hugs the hull side instead of reaching over the belt: {:.3}",
                b.max.x
            );
            assert!(b.min.x > 1.0, "and stays outboard of the tub wall: {:.3}", b.min.x);
        }
    }

    #[test]
    fn a_chamfered_box_has_no_raw_top_edges() {
        let mesh = chamfered_box(Vec3::new(1.0, 1.2, 0.5), Vec3::new(0.3, 0.15, 0.4), 0.04)
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("chamfered box is valid");
        // The chamfer planes must survive as real faces: some normals lie between axis directions.
        let bevelled = mesh
            .vertices()
            .iter()
            .any(|v| v.normal.y > 0.5 && (v.normal.x.abs() > 0.5 || v.normal.z.abs() > 0.5));
        assert!(bevelled, "the pressed bin needs 45-degree bevel faces, not raw box edges");
    }

    #[test]
    fn the_periscope_head_has_a_forward_raked_prism_face() {
        // The defect this locks: a plain box has only axis-aligned faces. The periscope must carry a
        // slanted prism face pointing forward and up (the raked glass), so it reads as a real device.
        let mesh = periscope(Vec3::new(0.34, 1.88, 0.42), Vec3::new(0.07, 0.06, 0.07))
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("periscope is valid");
        let raked = mesh.vertices().iter().any(|v| v.normal.y > 0.3 && v.normal.z > 0.3);
        assert!(raked, "periscope needs a forward-and-up raked prism face, not only box faces");
    }
}
