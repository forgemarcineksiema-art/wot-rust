//! Deterministic surface scatter: walk a fixed-order list of candidate slots and accept each that is
//! outside every exclusion zone and at least `min_spacing` from every prior acceptance. Order and
//! per-feature seed depend only on the request identity (never process state), so a bake reproduces.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

use crate::{FeatureKind, stable_hash};

/// A spherical keep-out region — armour seams, the gun socket, hatches, already-placed detail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExclusionZone {
    pub center: Vec3,
    pub radius: f32,
}

impl ExclusionZone {
    fn excludes(&self, p: Vec3) -> bool {
        (p - self.center).length() <= self.radius
    }
}

/// A candidate seat on the surface: where a feature could sit and which way it faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScatterSlot {
    pub position: Vec3,
    pub normal: Vec3,
}

/// A request to scatter one feature kind across an ordered list of candidate slots.
pub struct ScatterRequest<'a> {
    pub vehicle: &'a str,
    pub part: &'a str,
    pub feature: FeatureKind,
    pub slots: &'a [ScatterSlot],
    pub exclusions: &'a [ExclusionZone],
    pub min_spacing: f32,
}

/// An accepted placement, carrying its deterministic per-feature seed and ordinal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScatterPlacement {
    pub position: Vec3,
    pub normal: Vec3,
    pub seed: u64,
    pub ordinal: u32,
}

/// Walk the slots in order, accepting each that is outside every exclusion zone and at least
/// `min_spacing` from every prior acceptance. Deterministic: the result depends only on the request.
pub fn scatter(req: &ScatterRequest) -> Vec<ScatterPlacement> {
    let mut accepted: Vec<ScatterPlacement> = Vec::new();
    for slot in req.slots {
        if req.exclusions.iter().any(|z| z.excludes(slot.position)) {
            continue;
        }
        if req.min_spacing > 0.0
            && accepted.iter().any(|a| (a.position - slot.position).length() < req.min_spacing)
        {
            continue;
        }
        let ordinal = accepted.len() as u32;
        let seed = stable_hash(req.vehicle, req.part, req.feature, ordinal);
        accepted.push(ScatterPlacement {
            position: slot.position,
            normal: slot.normal,
            seed,
            ordinal,
        });
    }
    accepted
}

/// An array of `count` thin raised slats filling a `width` × `height` louvre panel centred at
/// `center` and facing `normal` — the engine-deck / radiator cover detail. Slats run across the
/// width and are stacked up the height, each standing `depth` proud of the panel.
pub fn louvre_slats(
    center: Vec3,
    normal: Vec3,
    width: f32,
    height: f32,
    depth: f32,
    count: u32,
    rake_rad: f32,
) -> GeometryMesh {
    let n = normal.normalize_or_zero();
    let n = if n == Vec3::ZERO { Vec3::Y } else { n };
    // WIDTH RUNS ACROSS THE OPENING AND HEIGHT RUNS UP IT — on a wall as much as on a deck.
    //
    // This used to be `n x Z` unconditionally, which is right for a horizontal panel (`n = Y`
    // gives across = X, stacking down Z) and turns a vertical one on its side: for `n = -X` it
    // returns +Y, so a cowl asking for 0.675 m of width across its face got 0.675 m of fin
    // standing UP. The T-54's exhaust grew five 675 mm blades out of a 220 mm box, through the
    // fender and down into the moving top run of the track. Prefer world up, and only fall back
    // to the cross product when the face IS the floor or the ceiling.
    let across = if n.cross(Vec3::Y).length() > 1.0e-3 {
        n.cross(Vec3::Y).normalize()
    } else if n.cross(Vec3::Z).length() > 1.0e-3 {
        n.cross(Vec3::Z).normalize()
    } else {
        Vec3::X
    };
    let up = n.cross(across).normalize();
    let count = count.max(1);
    let pitch = height / count as f32;
    let half_w = width * 0.5;
    let half_slat = pitch * 0.35;

    // The RAKE. A louvre is a plate tilted across the opening: that tilt is what lets air out
    // while keeping a downward view of the engine bay closed, and it is what makes the slats read
    // as a grille rather than as a ladder painted on a hole. The slats here were axis-aligned
    // boxes — flat — so the primitive named for real louvres did not make them.
    let (sin_rake, cos_rake) = rake_rad.sin_cos();
    let slat_up = up * cos_rake + n * sin_rake;
    let slat_out = n * cos_rake - up * sin_rake;

    let mut box_meshes = Vec::with_capacity(count as usize);
    for i in 0..count {
        let t = (i as f32 + 0.5) / count as f32 - 0.5;
        let slat_center = center + up * (t * height) + n * (depth * 0.5);
        box_meshes.push(plate_box(
            slat_center,
            across * half_w,
            slat_up * half_slat,
            slat_out * (depth * 0.5),
        ));
    }
    revolve::merge(&box_meshes).weld_and_smooth()
}

/// A hard-edged box centred at `c` with half-axes `du`, `dv`, `dw`. Each face authors its own
/// geometric normal so the welder keeps the corners crisp instead of rounding them.
pub(crate) fn plate_box(c: Vec3, du: Vec3, dv: Vec3, dw: Vec3) -> GeometryMesh {
    oriented_plate(c, du, dv, dw, MaterialRole::CastArmor)
}

/// [`plate_box`] with the CALLER's material — the same lesson `coaming` already carries: a
/// generator that hard-codes one steel hands every consumer that steel. The louvre slats shipped
/// as team-tinted armour because of it; a vision block's GLASS cannot be cast armour at all.
pub fn oriented_plate(
    c: Vec3,
    du: Vec3,
    dv: Vec3,
    dw: Vec3,
    material: MaterialRole,
) -> GeometryMesh {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    // (sign axis, in-plane half-axes) per face, wound CCW seen from outside.
    let faces =
        [(dw, du, dv), (-dw, dv, du), (du, dv, dw), (-du, dw, dv), (dv, dw, du), (-dv, du, dw)];
    for (out, a, b) in faces {
        let normal = out.normalize_or_zero();
        let base = vertices.len() as u32;
        for corner in [out - a - b, out + a - b, out + a + b, out - a + b] {
            vertices.push(GeometryVertex::new(
                c + corner,
                normal,
                material,
                SmoothingGroup::hard_edges(),
            ));
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    GeometryMesh::new(vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots() -> Vec<ScatterSlot> {
        (0..5)
            .map(|i| ScatterSlot { position: Vec3::new(i as f32 * 0.1, 0.0, 0.0), normal: Vec3::Y })
            .collect()
    }

    #[test]
    fn scatter_is_deterministic_in_order_and_seed() {
        let s = slots();
        let req = ScatterRequest {
            vehicle: "t54",
            part: "glacis",
            feature: FeatureKind::Bolt,
            slots: &s,
            exclusions: &[],
            min_spacing: 0.0,
        };
        let a = scatter(&req);
        let b = scatter(&req);
        assert_eq!(a, b, "same request scatters identically");
        assert_eq!(a.len(), 5);
        assert_eq!(a[0].ordinal, 0);
        assert_ne!(a[0].seed, a[1].seed, "each ordinal gets its own seed");
    }

    #[test]
    fn exclusion_zones_drop_slots_inside_them() {
        let s = slots();
        let req = ScatterRequest {
            vehicle: "t54",
            part: "glacis",
            feature: FeatureKind::Bolt,
            slots: &s,
            exclusions: &[ExclusionZone { center: Vec3::new(0.2, 0.0, 0.0), radius: 0.05 }],
            min_spacing: 0.0,
        };
        let placed = scatter(&req);
        assert_eq!(placed.len(), 4, "the slot at x=0.2 is excluded");
        assert!(placed.iter().all(|p| (p.position.x - 0.2).abs() > 1.0e-4));
        // Ordinals stay contiguous so seeds remain stable regardless of which slots were dropped.
        assert_eq!(placed.iter().map(|p| p.ordinal).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn min_spacing_thins_clustered_slots() {
        let s = slots();
        let req = ScatterRequest {
            vehicle: "t54",
            part: "glacis",
            feature: FeatureKind::Bolt,
            slots: &s,
            exclusions: &[],
            min_spacing: 0.25,
        };
        let placed = scatter(&req);
        assert!(placed.len() < 5, "spacing rejects neighbours within 0.25");
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                assert!((a.position - b.position).length() >= 0.25);
            }
        }
    }

    #[test]
    fn a_louvre_array_is_clean_and_spans_its_panel() {
        let mesh = louvre_slats(Vec3::ZERO, Vec3::Y, 0.6, 0.4, 0.02, 4, 0.0);
        let b = mesh.bounds().expect("louvre bounds");
        assert!(b.max.y > 0.0, "slats stand proud along +Y");
        assert!((b.max.x - 0.3).abs() < 0.02 && (b.min.x + 0.3).abs() < 0.02, "spans the width");
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean louvre array");
    }

    /// The rake is the whole point of the primitive, so it gets its own lock: a raked array is
    /// DEEPER along its own normal than a flat one of the same slats, because each plate now
    /// leans across the opening instead of lying square to it.
    #[test]
    fn a_raked_louvre_leans_across_its_opening() {
        let flat = louvre_slats(Vec3::ZERO, Vec3::Y, 0.6, 0.4, 0.02, 4, 0.0);
        let raked = louvre_slats(Vec3::ZERO, Vec3::Y, 0.6, 0.4, 0.02, 4, 0.6);
        let depth = |mesh: &GeometryMesh| {
            let b = mesh.bounds().expect("bounds");
            b.max.y - b.min.y
        };
        assert!(
            depth(&raked) > depth(&flat) * 1.5,
            "a leaning plate reaches further across the opening: {:.4} vs {:.4}",
            depth(&raked),
            depth(&flat)
        );
        raked.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH).expect("clean raked array");
    }
}
