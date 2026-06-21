//! Cross-section **loft** kernel: skin a stack of horizontal cross-sections into one watertight cast
//! shell. This is the controlled-surface answer to organic castings (turrets, masks) that metaball
//! SDF composition cannot express cleanly — a lofted shell is *designed* at every station, so it
//! reads as one continuous casting from every angle instead of a pile of blended spheres.
//!
//! Generic and renderer-free: it knows nothing about any specific vehicle and produces a plain
//! [`vehicle_geometry::GeometryMesh`]. Rounded cross-sections give a Soviet cast dome; rectangular
//! ones (high superellipse exponent) give a boxy turret — one kernel for every vehicle family.
//!
//! Normals are not authored here: the shell is emitted with a smooth [`SmoothingGroup`] and run
//! through [`GeometryMesh::weld_and_smooth`], which rebuilds vertex normals from the faces.

use std::f32::consts::{PI, TAU};

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

/// A closed horizontal cross-section at height `y`: a superellipse in the XZ plane with separate
/// front (`+Z`) and rear (`-Z`) half-lengths, so a casting can read front-heavy with a tapered rear
/// bustle. `exponent` is the superellipse fullness (`2.0` = ellipse, `>2.0` = fuller "shoulders"
/// approaching a rounded rectangle).
#[derive(Debug, Clone, Copy)]
pub struct CrossSection {
    pub y: f32,
    pub half_width: f32,
    pub half_len_front: f32,
    pub half_len_rear: f32,
    pub z_center: f32,
    pub exponent: f32,
}

impl CrossSection {
    /// A symmetric (front == rear) section — the common case for a plain dome ring.
    pub fn symmetric(y: f32, half_width: f32, half_len: f32, z_center: f32, exponent: f32) -> Self {
        Self {
            y,
            half_width,
            half_len_front: half_len,
            half_len_rear: half_len,
            z_center,
            exponent,
        }
    }

    /// The outline point at azimuth `t` (radians; `0` = `+X`, `PI/2` = `+Z` front), before bumps.
    fn point(&self, t: f32) -> Vec3 {
        let e = 2.0 / self.exponent;
        let (st, ct) = t.sin_cos();
        let x = self.half_width * superlerp(ct, e);
        let hl = if st >= 0.0 { self.half_len_front } else { self.half_len_rear };
        let z = self.z_center + hl * superlerp(st, e);
        Vec3::new(x, self.y, z)
    }
}

/// Superellipse coordinate: `sign(c) * |c|^e`.
fn superlerp(c: f32, e: f32) -> f32 {
    c.signum() * c.abs().powf(e)
}

/// An additive radial push localised in azimuth and height: positive bulges the surface outward
/// (a cheek or brow), negative pulls it inward (a gun embrasure recess). The push is part of the one
/// lofted surface — not a stuck-on primitive — so swellings and recesses stay continuous castings.
#[derive(Debug, Clone, Copy)]
pub struct AzimuthBump {
    /// Centre azimuth in radians (`PI/2` = front, `0` / `PI` = the two sides).
    pub azimuth: f32,
    /// Gaussian half-spread in azimuth (radians).
    pub az_width: f32,
    /// Centre height.
    pub y: f32,
    /// Gaussian half-spread in height.
    pub y_width: f32,
    /// Peak radial push in metres (negative = recess).
    pub amount: f32,
}

impl AzimuthBump {
    fn push(&self, t: f32, y: f32) -> f32 {
        let mut d = (t - self.azimuth).rem_euclid(TAU);
        if d > PI {
            d -= TAU;
        }
        let az = (-(d / self.az_width).powi(2)).exp();
        let h = (-((y - self.y) / self.y_width).powi(2)).exp();
        self.amount * az * h
    }
}

/// How the ends of a loft are closed: an apex point each end fans to, or `None` to leave it open.
#[derive(Debug, Clone, Copy, Default)]
pub struct Caps {
    pub bottom: Option<Vec3>,
    pub top: Option<Vec3>,
}

/// Everything needed to skin one shell.
pub struct LoftSpec<'a> {
    /// Cross-sections from bottom to top (at least two).
    pub sections: &'a [CrossSection],
    /// Outward/inward radial modulations applied to every station.
    pub bumps: &'a [AzimuthBump],
    /// Azimuth samples per ring (mesh resolution around the shell).
    pub segments: usize,
    pub caps: Caps,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

/// Skin a stack of cross-sections into a watertight cast shell.
pub fn loft(spec: &LoftSpec) -> GeometryMesh {
    let n = spec.segments.max(3);
    let rings = spec.sections.len();
    assert!(rings >= 2, "a loft needs at least two cross-sections");

    let mut positions: Vec<Vec3> = Vec::with_capacity(rings * n + 2);
    for section in spec.sections {
        for i in 0..n {
            let t = TAU * i as f32 / n as f32;
            let mut p = section.point(t);
            let push: f32 = spec.bumps.iter().map(|b| b.push(t, section.y)).sum();
            if push != 0.0 {
                let radial = Vec3::new(p.x, 0.0, p.z - section.z_center).normalize_or_zero();
                p += radial * push;
            }
            positions.push(p);
        }
    }

    let ring_base = |r: usize| (r * n) as u32;
    let mut indices: Vec<u32> = Vec::new();
    for r in 0..rings - 1 {
        for i in 0..n {
            let i1 = (i + 1) % n;
            let a = ring_base(r) + i as u32;
            let b = ring_base(r) + i1 as u32;
            let c = ring_base(r + 1) + i1 as u32;
            let d = ring_base(r + 1) + i as u32;
            indices.extend_from_slice(&[a, d, c, a, c, b]);
        }
    }

    if let Some(apex) = spec.caps.bottom {
        let centre = positions.len() as u32;
        positions.push(apex);
        for i in 0..n {
            let i1 = (i + 1) % n;
            indices.extend_from_slice(&[centre, ring_base(0) + i as u32, ring_base(0) + i1 as u32]);
        }
    }
    if let Some(apex) = spec.caps.top {
        let centre = positions.len() as u32;
        positions.push(apex);
        let last = rings - 1;
        for i in 0..n {
            let i1 = (i + 1) % n;
            indices.extend_from_slice(&[
                centre,
                ring_base(last) + i1 as u32,
                ring_base(last) + i as u32,
            ]);
        }
    }

    let vertices = positions
        .iter()
        .map(|&p| GeometryVertex::new(p, Vec3::ZERO, spec.material, spec.smoothing))
        .collect();
    // Smooth normals are rebuilt from the faces here; the placeholder zero normals above are replaced.
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dome_sections() -> Vec<CrossSection> {
        vec![
            CrossSection::symmetric(0.00, 0.84, 0.92, 0.0, 2.8),
            CrossSection::symmetric(0.16, 0.92, 1.00, -0.02, 2.8),
            CrossSection::symmetric(0.36, 0.82, 0.86, -0.07, 2.8),
            CrossSection::symmetric(0.55, 0.50, 0.60, -0.12, 2.8),
        ]
    }

    fn dome(bumps: &[AzimuthBump]) -> GeometryMesh {
        loft(&LoftSpec {
            sections: &dome_sections(),
            bumps,
            segments: 48,
            caps: Caps {
                bottom: Some(Vec3::new(0.0, 0.0, 0.0)),
                top: Some(Vec3::new(0.0, 0.55, -0.06)),
            },
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(2),
        })
    }

    /// A capped loft is a closed 2-manifold: every undirected edge is shared by exactly two faces.
    #[test]
    fn capped_loft_is_a_closed_manifold() {
        let mesh = dome(&[]);
        let mut edges: std::collections::HashMap<(u32, u32), u32> =
            std::collections::HashMap::new();
        for tri in mesh.indices().chunks_exact(3) {
            for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert!(!edges.is_empty());
        assert!(
            edges.values().all(|&count| count == 2),
            "a closed shell shares every edge between exactly two faces"
        );
    }

    #[test]
    fn welded_normals_are_finite_and_unit() {
        let mesh = dome(&[]);
        for v in mesh.vertices() {
            assert!(v.normal.is_finite() && (v.normal.length() - 1.0).abs() < 1.0e-3);
        }
    }

    #[test]
    fn a_wide_flat_section_reads_wider_than_tall() {
        let mesh = dome(&[]);
        let b = mesh.bounds().expect("non-empty");
        assert!(b.max.x - b.min.x > b.max.y - b.min.y, "the dome is wider than it is tall");
    }

    #[test]
    fn a_cheek_bump_pushes_the_surface_out_only_where_aimed() {
        let plain = dome(&[]).bounds().unwrap();
        // A cheek on the +X side at mid height.
        let bumped = dome(&[AzimuthBump {
            azimuth: 0.0,
            az_width: 0.4,
            y: 0.2,
            y_width: 0.2,
            amount: 0.18,
        }])
        .bounds()
        .unwrap();
        assert!(bumped.max.x > plain.max.x + 0.10, "the cheek bulges the aimed side outward");
        assert!((bumped.max.z - plain.max.z).abs() < 0.02, "the front is left untouched");
    }

    #[test]
    fn the_kernel_is_deterministic() {
        assert_eq!(dome(&[]), dome(&[]));
    }
}
