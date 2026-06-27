//! Bake-only visual deformation (renderer-free): a *strictly* constrained displacement applied to a
//! visual `GeometryMesh` **after** its base shape has passed audit. This breaks the dead symmetry of
//! procedural castings — a faint cast asymmetry, a shallow dent, light surface wear — without ever
//! touching gameplay truth.
//!
//! # Contract
//! Deformation takes and returns only a [`GeometryMesh`]. It never sees a `HitboxProfile`, armour
//! facets, mount frames, part anchors or module locations, so it *cannot* move them. Every vertex is
//! displaced along its own normal by at most `tolerance` metres, clamped into the part's authored
//! gameplay volume, and the result is re-smoothed so normals stay finite. The displacement is a pure
//! function of vertex position and the spec `seed`, so a bake reproduces byte-for-byte.

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex};

/// The sanctioned visual deformations. Battle damage is deliberately *not* here: the first use is a
/// faint cast asymmetry on a smooth turret, never destruction that would imply armour change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeformationKind {
    /// Low-frequency bidirectional swell — the uneven wall of a sand-cast turret.
    CastAsymmetry,
    /// A smooth inward bowl — a shallow dent, deepest at the centre.
    ShallowDent,
    /// Fine high-frequency bidirectional roughness — casting pits and wear.
    SurfaceWear,
}

/// A single deformation: where it acts, how far it reaches, how hard it pushes, and its seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeformationSpec {
    pub kind: DeformationKind,
    pub center: Vec3,
    pub radius: f32,
    pub amplitude: f32,
    pub seed: u64,
}

/// Apply `spec` to a copy of `mesh`, displacing each vertex within `spec.radius` of `spec.center`
/// along its normal. The per-vertex displacement is clamped to `± tolerance` metres so it can never
/// leave the authored gameplay volume; normals are rebuilt afterwards. Pure in `(position, seed)`.
pub fn deform(mesh: &GeometryMesh, spec: &DeformationSpec, tolerance: f32) -> GeometryMesh {
    let limit = clean_tolerance(tolerance);
    if limit == 0.0 || !spec.radius.is_finite() || spec.radius <= 0.0 {
        return mesh.clone();
    }
    let vertices: Vec<GeometryVertex> = mesh
        .vertices()
        .iter()
        .map(|v| {
            let signed = displacement(spec, v.position).clamp(-limit, limit);
            GeometryVertex { position: v.position + v.normal * signed, ..*v }
        })
        .collect();
    // Re-smooth so the displaced surface gets finite, consistent normals (and stays welded where the
    // input was welded — the displacement is position-keyed so coincident vertices move together).
    GeometryMesh::new(vertices, mesh.indices().to_vec()).weld_and_smooth()
}

/// The signed displacement (in metres, before tolerance clamping) for a vertex at `position`.
fn displacement(spec: &DeformationSpec, position: Vec3) -> f32 {
    let d = (position - spec.center).length();
    if d >= spec.radius {
        return 0.0;
    }
    let weight = falloff(d / spec.radius);
    match spec.kind {
        DeformationKind::CastAsymmetry => {
            spec.amplitude * weight * unit_signed(spec.seed, position, 64.0)
        }
        DeformationKind::ShallowDent => -spec.amplitude * weight,
        DeformationKind::SurfaceWear => {
            spec.amplitude * weight * unit_signed(spec.seed, position, 512.0)
        }
    }
}

/// Smooth radial weight: 1 at the centre, easing to 0 at the rim (so deformation never steps).
fn falloff(x: f32) -> f32 {
    let s = (1.0 - x).clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

/// A deterministic value in `[-1, 1]` keyed on `seed` and the position quantised to a `scale` grid.
/// A coarse `scale` gives smooth low-frequency lobes; a fine `scale` gives high-frequency roughness.
fn unit_signed(seed: u64, position: Vec3, scale: f32) -> f32 {
    let q = |c: f32| (c * scale).round() as i64 as u64;
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for word in [q(position.x), q(position.y), q(position.z)] {
        for byte in word.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    // Map the high bits to [-1, 1].
    (h >> 40) as f32 / (1u64 << 23) as f32 * 2.0 - 1.0
}

/// A non-negative, finite tolerance magnitude — a stray sign or NaN reads as "no deformation".
fn clean_tolerance(tolerance: f32) -> f32 {
    if tolerance.is_finite() { tolerance.abs() } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicle_geometry::MaterialRole;

    /// A smooth revolved sphere of `radius`, the stand-in for a cast turret shell. Revolve gives it
    /// clean apex caps (no degenerate pole triangles), so it is a genuine closed manifold.
    fn sphere(radius: f32) -> GeometryMesh {
        let stacks = 16_usize;
        let profile: Vec<(f32, f32)> = (0..=stacks)
            .map(|i| {
                let y = -1.0 + 2.0 * i as f32 / stacks as f32;
                (radius * y, radius * (1.0 - y * y).max(0.0).sqrt())
            })
            .collect();
        revolve::revolve(
            Vec3::Y,
            &profile,
            24,
            MaterialRole::CastArmor,
            vehicle_geometry::SmoothingGroup(9),
        )
    }

    fn cast_spec() -> DeformationSpec {
        DeformationSpec {
            kind: DeformationKind::CastAsymmetry,
            center: Vec3::ZERO,
            radius: 2.0,
            amplitude: 0.03,
            seed: 0x1234,
        }
    }

    #[test]
    fn deformation_is_deterministic_in_position_and_seed() {
        let base = sphere(1.0);
        let a = deform(&base, &cast_spec(), 0.02);
        let b = deform(&base, &cast_spec(), 0.02);
        assert_eq!(a.vertices().len(), b.vertices().len());
        for (p, q) in a.vertices().iter().zip(b.vertices()) {
            assert_eq!(p.position, q.position, "same spec deforms identically");
        }
    }

    #[test]
    fn displacement_is_contained_within_tolerance() {
        let base = sphere(1.0);
        let tolerance = 0.02;
        let out = deform(&base, &cast_spec(), tolerance);
        for (before, after) in base.vertices().iter().zip(out.vertices()) {
            let moved = (after.position - before.position).length();
            assert!(moved <= tolerance + 1.0e-5, "vertex moved {moved} > tolerance {tolerance}");
        }
        // The expanded bounds cannot exceed the original by more than the tolerance.
        let (b0, b1) = (base.bounds().unwrap(), out.bounds().unwrap());
        assert!((b1.max - b0.max).max_element() <= tolerance + 1.0e-4);
    }

    #[test]
    fn every_normal_stays_finite_after_resmoothing() {
        let out = deform(&sphere(1.0), &cast_spec(), 0.02);
        for v in out.vertices() {
            assert!(v.normal.is_finite() && (v.normal.length() - 1.0).abs() < 1.0e-3);
        }
        out.validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH)
            .expect("a deformed smooth sphere stays a closed manifold");
    }

    #[test]
    fn a_zero_tolerance_is_a_no_op() {
        let base = sphere(1.0);
        let out = deform(&base, &cast_spec(), 0.0);
        for (a, b) in base.vertices().iter().zip(out.vertices()) {
            assert_eq!(a.position, b.position, "no tolerance means no deformation");
        }
    }

    #[test]
    fn a_shallow_dent_pushes_inward() {
        let base = sphere(1.0);
        let spec = DeformationSpec {
            kind: DeformationKind::ShallowDent,
            center: Vec3::new(0.0, 1.0, 0.0),
            radius: 0.5,
            amplitude: 0.04,
            seed: 7,
        };
        let out = deform(&base, &spec, 0.03);
        // The pole vertex nearest the dent centre must have moved inward (toward the origin).
        let top_before = base.vertices().iter().map(|v| v.position.y).fold(f32::MIN, f32::max);
        let top_after = out.vertices().iter().map(|v| v.position.y).fold(f32::MIN, f32::max);
        assert!(top_after < top_before, "the dent lowers the top of the shell");
    }
}
