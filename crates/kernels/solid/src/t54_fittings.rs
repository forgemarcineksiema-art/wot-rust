//! T-54 fender furniture and small pressed fittings as convex solids — split from `t54.rs` to keep
//! each file within the reviewability budget. Every dimension is read from the blueprint's
//! [`FenderVisual`] / [`DetailVisual`]; nothing here invents a tank dimension.

use game_core::{DetailVisual, FenderVisual};
use glam::Vec3;

use crate::{ConvexSolid, Plane};

/// An axis-aligned box with 45° chamfers on its four top edges and four vertical edges — the
/// pressed-steel read of fender stowage (fuel tanks, bins) instead of a raw primitive box.
/// `chamfer` is clamped so the cuts can never cross for any sane bin.
///
/// A chamfer of zero (or one finer than the corner-merge epsilon) asks for a PLAIN box, and
/// that is what it gets: each chamfer plane would otherwise pass exactly through a box edge,
/// leaving a two-corner face ring that `to_mesh` rejects as degenerate. "No chamfer" is a sane
/// request from a caller sweeping the parameter — not a build error.
pub fn chamfered_box(center: Vec3, half: Vec3, chamfer: f32) -> ConvexSolid {
    /// Below this the cut plane is indistinguishable from the edge it would trim: the corner
    /// dedup in `ConvexSolid::to_mesh` merges at 1e-4, so anything finer is not a chamfer.
    const MIN_CHAMFER: f32 = 1.0e-3;
    let c = chamfer.clamp(0.0, 0.45 * half.min_element());
    if c < MIN_CHAMFER {
        return ConvexSolid::box_at(center, half);
    }
    let sqrt2 = std::f32::consts::SQRT_2;
    let face = |n: Vec3, off: f32| Plane::new(n, off + n.dot(center));
    let mut planes = vec![
        face(Vec3::X, half.x),
        face(-Vec3::X, half.x),
        face(Vec3::Y, half.y),
        face(-Vec3::Y, half.y),
        face(Vec3::Z, half.z),
        face(-Vec3::Z, half.z),
    ];
    for sx in [-1.0_f32, 1.0] {
        // Top edges along Z and along X, then the vertical corner edges.
        planes.push(face(Vec3::new(sx, 1.0, 0.0) / sqrt2, (half.x + half.y - c) / sqrt2));
        planes.push(face(Vec3::new(0.0, 1.0, sx) / sqrt2, (half.z + half.y - c) / sqrt2));
        for sz in [-1.0_f32, 1.0] {
            planes.push(face(Vec3::new(sx, 0.0, sz) / sqrt2, (half.x + half.z - c) / sqrt2));
        }
    }
    ConvexSolid::new(planes)
}

/// One sloping fender end section (front mudguard dropping over the idler, or the rear flap over
/// the sprocket): a thin plate falling from the flat fender run down toward the track end, as the
/// references show. `sign` is +1 for the front section, -1 for the rear.
pub fn t54_fender_slope(side_x: f32, fender: &FenderVisual, sign: f32) -> ConvexSolid {
    let top = Vec3::new(side_x, fender.center_y, sign * fender.half.z);
    let bottom = Vec3::new(side_x, fender.center_y - 0.33, sign * (fender.half.z + 0.31));
    let along = (bottom - top).normalize();
    let normal = Vec3::X.cross(along).normalize();
    let mid = 0.5 * (top + bottom);
    let x_out = side_x + side_x.signum() * fender.half.x;
    let x_in = side_x - side_x.signum() * fender.half.x;
    ConvexSolid::new(vec![
        Plane::new(normal, normal.dot(mid) + 0.015),
        Plane::new(-normal, -(normal.dot(mid) - 0.015)),
        Plane::new(along, along.dot(bottom)),
        Plane::new(-along, -along.dot(top)),
        Plane::new(Vec3::X, x_out.max(x_in)),
        Plane::new(-Vec3::X, -x_out.min(x_in)),
    ])
}

/// The fender (mudguard) above one track run, split into bolted sections along its length with thin
/// seam gaps, so it reads as pressed plates rather than one continuous slab. Visual only; the overall
/// footprint is unchanged. The downturned outer lip ([`t54_fender_lip`]) runs the full edge.
pub fn t54_fender_segments(side_x: f32, fender: &FenderVisual) -> Vec<ConvexSolid> {
    const SEGMENTS: usize = 4;
    let seam = 0.03_f32;
    let seg_half = Vec3::new(fender.half.x, fender.half.y, fender.half.z / SEGMENTS as f32 - seam);
    (0..SEGMENTS)
        .map(|i| {
            let t = (i as f32 + 0.5) / SEGMENTS as f32;
            let z = -fender.half.z + t * 2.0 * fender.half.z;
            ConvexSolid::box_at(Vec3::new(side_x, fender.center_y, z), seg_half)
        })
        .collect()
}

/// Thin gusset plates under the fender span, evenly spaced along its run — the supports that carry
/// the shelf over the exposed track. Each is a cross-fender plate hanging just below the shelf
/// (clear of the moving top track run). Visual only, at the close-up detail tier.
pub fn t54_fender_brackets(side_x: f32, fender: &FenderVisual) -> Vec<ConvexSolid> {
    const BRACKETS: usize = 5;
    let drop = 0.08_f32;
    let bottom = fender.center_y - fender.half.y;
    let half = Vec3::new(fender.half.x - 0.04, drop * 0.5, 0.015);
    (0..BRACKETS)
        .map(|i| {
            let t = (i as f32 + 0.5) / BRACKETS as f32;
            let z = -fender.half.z + t * 2.0 * fender.half.z;
            ConvexSolid::box_at(Vec3::new(side_x, bottom - drop * 0.5, z), half)
        })
        .collect()
}

/// The louvered exhaust box on the left fender at the engine bay — a chamfered armoured housing,
/// the clean factory exhaust, not a sooted pipe.
pub fn t54_exhaust_housing(d: &DetailVisual) -> ConvexSolid {
    chamfered_box(d.exhaust_center, d.exhaust_half, 0.03)
}

/// A thin downturned fender lip running the length of one fender's outer edge, at world `side_x`.
/// It drops just below the fender plate so the mudguard reads as a folded-edge pressing.
pub fn t54_fender_lip(side_x: f32, fender: &FenderVisual, d: &DetailVisual) -> ConvexSolid {
    let edge_x = side_x.signum() * (side_x.abs() + fender.half.x);
    let bottom = fender.center_y - fender.half.y;
    let center = Vec3::new(edge_x, bottom - d.fender_lip_drop * 0.5, 0.0);
    let half = Vec3::new(d.fender_lip_thickness * 0.5, d.fender_lip_drop * 0.5, fender.half.z);
    ConvexSolid::box_at(center, half)
}

/// A periscope head: a small housing box whose front-top edge is sliced by a forward-and-up plane,
/// giving the raked prism (viewing-glass) face that reads as a real periscope rather than a plain
/// block. The slant looks forward (+z); mirror the centre in x for the opposite-hand device.
pub fn t54_periscope(center: Vec3, half: Vec3) -> ConvexSolid {
    // A 45-degree chamfer of depth `s` off the front-top edge: the prism glass rakes back as it
    // rises. `s` is a fraction of the head so the cut never crosses the box for any sane periscope.
    let s = 0.7 * half.y.min(half.z);
    let normal = Vec3::new(0.0, 1.0, 1.0);
    let through = Vec3::new(center.x, center.y + half.y - s, center.z + half.z);
    ConvexSolid::box_at(center, half).clipped_by(Plane::new(normal, normal.dot(through)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::{MaterialRole, SmoothingGroup};

    fn fender() -> FenderVisual {
        game_core::VehicleBlueprint::for_vehicle(game_core::VehicleKind::T54_1951)
            .unwrap()
            .visual_detail()
            .unwrap()
            .fender
    }

    #[test]
    fn the_fender_splits_into_separated_sections() {
        let f = fender();
        let segs = t54_fender_segments(1.5, &f);
        assert!(segs.len() >= 3, "fender reads as bolted sections, got {}", segs.len());
        // The seam gaps mean the sections' lengths sum to less than the whole fender span.
        let covered: f32 = segs
            .iter()
            .map(|s| {
                let b = s
                    .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
                    .expect("fender segment is valid")
                    .bounds();
                let b = b.expect("non-empty segment");
                b.max.z - b.min.z
            })
            .sum();
        assert!(covered < 2.0 * f.half.z - 0.05, "seam gaps separate the fender sections");
    }

    #[test]
    fn fender_brackets_hang_below_the_fender_over_the_track() {
        let f = fender();
        let brackets = t54_fender_brackets(1.345, &f);
        assert!(brackets.len() >= 3, "several brackets along the run");
        let bottom = f.center_y - f.half.y;
        for bracket in &brackets {
            let b = bracket
                .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
                .expect("bracket is valid")
                .bounds()
                .expect("non-empty bracket");
            assert!(b.min.y < bottom - 0.03, "bracket hangs below the fender plate");
            assert!(b.min.y > 0.95, "bracket stays clear of the moving top track run");
            assert!(b.max.x > 1.3, "bracket spans the fender over the track band");
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
    fn the_fender_slope_drops_over_the_track_end() {
        let f = fender();
        let front = t54_fender_slope(1.345, &f, 1.0)
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("front mudguard is valid")
            .bounds()
            .expect("non-empty mudguard");
        assert!(front.max.z > f.half.z + 0.2, "the mudguard reaches out past the fender run");
        assert!(front.min.y < f.center_y - 0.25, "the mudguard drops down over the idler");
    }

    #[test]
    fn the_periscope_head_has_a_forward_raked_prism_face() {
        // The defect this locks: a plain box has only axis-aligned faces. The periscope must carry a
        // slanted prism face pointing forward and up (the raked glass), so it reads as a real device.
        let mesh = t54_periscope(Vec3::new(0.34, 1.88, 0.42), Vec3::new(0.07, 0.06, 0.07))
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("periscope is valid");
        let raked = mesh.vertices().iter().any(|v| v.normal.y > 0.3 && v.normal.z > 0.3);
        assert!(raked, "periscope needs a forward-and-up raked prism face, not only box faces");
    }
}
