//! **Cast loft** kernel: skin a stack of superelliptic horizontal stations into one watertight cast
//! shell. This is the controlled-surface answer to *designed castings* (turrets, masks) that
//! metaball SDF composition cannot express cleanly — the shell is designed at every station, so it
//! reads as one continuous casting from every angle instead of a pile of blended spheres.
//!
//! This is deliberately distinct from the generic [`vehicle_geometry::LoftSpec`]: that is an
//! arbitrary convex 2D section swept along a cardinal axis, for fabrication and hull-like solids,
//! whereas a cast loft is superelliptic horizontal stations plus localized cast shaping. Do not put
//! a cast turret in a generic convex-hull loft, and do not use this superellipse API for flat
//! armour plates.
//!
//! Generic and renderer-free: it knows nothing about any specific vehicle and produces a plain
//! [`vehicle_geometry::GeometryMesh`]. Rounded stations give a Soviet cast dome; fuller ones (high
//! superellipse exponent) give a boxy turret — one kernel for every vehicle family.
//!
//! Normals are not authored here: the shell is emitted with a smooth [`SmoothingGroup`] and run
//! through [`GeometryMesh::weld_and_smooth`], which rebuilds vertex normals from the faces.

use std::f32::consts::{PI, TAU};

use glam::Vec3;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

mod error;

pub use error::{CastLoftError, try_build_cast_loft};

/// A closed horizontal cross-section at height `y`: a superellipse in the XZ plane with separate
/// front (`+Z`) and rear (`-Z`) half-lengths, so a casting can read front-heavy with a tapered rear
/// bustle. `exponent` is the superellipse fullness (`2.0` = ellipse, `>2.0` = fuller "shoulders"
/// approaching a rounded rectangle).
#[derive(Debug, Clone, Copy)]
pub struct CastSection {
    pub y: f32,
    pub half_width: f32,
    pub half_len_front: f32,
    pub half_len_rear: f32,
    pub z_center: f32,
    pub exponent: f32,
}

impl CastSection {
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
pub struct CastBump {
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
    /// How sharply the feature's walls stand up, as the exponent of a super-Gaussian.
    ///
    /// `2.0` is the plain Gaussian this kernel has always used: a soft dish, right for a
    /// casting's swells and hollows. Higher exponents flatten the FLOOR and steepen the WALLS
    /// toward a plateau — 6 already reads as a pocket with a rim rather than a dimple, and that
    /// is what a gun embrasure is: a narrow aperture cut into a casting, not a dent pressed
    /// into it.
    ///
    /// A sharp feature also has to be RESOLVED: the walls only exist if there are stations
    /// through them, which is why the validator refuses a bump narrower than its local station
    /// spacing. Steepness is not a substitute for stations.
    pub falloff_exponent: f32,
}

impl CastBump {
    /// A bump with the kernel's classic Gaussian falloff — a soft cast swell or hollow.
    pub fn gaussian(azimuth: f32, az_width: f32, y: f32, y_width: f32, amount: f32) -> Self {
        Self { azimuth, az_width, y, y_width, amount, falloff_exponent: 2.0 }
    }

    /// The same feature with steep walls and a flat floor: an aperture rather than a dish.
    pub fn plateau(
        azimuth: f32,
        az_width: f32,
        y: f32,
        y_width: f32,
        amount: f32,
        exponent: f32,
    ) -> Self {
        Self { azimuth, az_width, y, y_width, amount, falloff_exponent: exponent.max(2.0) }
    }

    fn push(&self, t: f32, y: f32) -> f32 {
        let mut d = (t - self.azimuth).rem_euclid(TAU);
        if d > PI {
            d -= TAU;
        }
        // Super-Gaussian: |x|^n instead of x^2. n = 2 is the original curve exactly.
        let n =
            if self.falloff_exponent.is_finite() { self.falloff_exponent.max(2.0) } else { 2.0 };
        let az = (-(d / self.az_width).abs().powf(n)).exp();
        let h = (-((y - self.y) / self.y_width).abs().powf(n)).exp();
        self.amount * az * h
    }
}

/// How one end of a cast loft is closed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CastCap {
    /// Emit no end faces; the shell stays open at this end.
    #[default]
    Open,
    /// Fan flat from the centroid of the terminal ring, in that ring's station plane — a flat lid
    /// with no artificial spike.
    Planar,
    /// Fan to an explicit apex point, for a domed or pointed end.
    Apex(Vec3),
}

/// How both ends of a cast loft are closed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CastCaps {
    pub bottom: CastCap,
    pub top: CastCap,
}

/// Everything needed to skin one shell.
pub struct CastLoftSpec<'a> {
    /// Cross-sections from bottom to top (at least two).
    pub sections: &'a [CastSection],
    /// Outward/inward radial modulations applied to every station.
    pub bumps: &'a [CastBump],
    /// Azimuth samples per ring (mesh resolution around the shell).
    pub segments: usize,
    pub caps: CastCaps,
    pub material: MaterialRole,
    pub smoothing: SmoothingGroup,
}

/// Skin a stack of cross-sections into a watertight cast shell, assuming the spec is already valid.
/// Production callers go through [`try_build_cast_loft`], which validates the spec first; this
/// unchecked builder stays crate-internal for that wrapper and the tests.
pub(crate) fn build_cast_loft(spec: &CastLoftSpec) -> GeometryMesh {
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

    // The bottom cap fans with the ring's natural winding; the top cap reverses it so both end
    // faces point away from the shell body.
    add_cap(&mut positions, &mut indices, spec.caps.bottom, ring_base(0), n, false);
    add_cap(&mut positions, &mut indices, spec.caps.top, ring_base(rings - 1), n, true);

    let vertices = positions
        .iter()
        .map(|&p| GeometryVertex::new(p, Vec3::ZERO, spec.material, spec.smoothing))
        .collect();
    // Smooth normals are rebuilt from the faces here; the placeholder zero normals above are replaced.
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}

/// Close one end of the shell according to `cap`. `ring_start` is the first vertex index of the
/// terminal ring; `reversed` flips the fan winding so the top end faces away from the body too.
fn add_cap(
    positions: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    cap: CastCap,
    ring_start: u32,
    n: usize,
    reversed: bool,
) {
    let centre_point = match cap {
        CastCap::Open => return,
        CastCap::Apex(apex) => apex,
        // The terminal ring's points all share the station's y, so their centroid lies in that
        // station plane — a true flat lid.
        CastCap::Planar => {
            let start = ring_start as usize;
            positions[start..start + n].iter().fold(Vec3::ZERO, |acc, &p| acc + p) / n as f32
        }
    };
    let centre = positions.len() as u32;
    positions.push(centre_point);
    for i in 0..n {
        let i1 = ((i + 1) % n) as u32;
        let (a, b) = (ring_start + i as u32, ring_start + i1);
        if reversed {
            indices.extend_from_slice(&[centre, b, a]);
        } else {
            indices.extend_from_slice(&[centre, a, b]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dome_sections() -> Vec<CastSection> {
        vec![
            CastSection::symmetric(0.00, 0.84, 0.92, 0.0, 2.8),
            CastSection::symmetric(0.16, 0.92, 1.00, -0.02, 2.8),
            CastSection::symmetric(0.36, 0.82, 0.86, -0.07, 2.8),
            CastSection::symmetric(0.55, 0.50, 0.60, -0.12, 2.8),
        ]
    }

    const SEGMENTS: usize = 48;

    fn loft_with(caps: CastCaps, bumps: &[CastBump]) -> GeometryMesh {
        build_cast_loft(&CastLoftSpec {
            sections: &dome_sections(),
            bumps,
            segments: SEGMENTS,
            caps,
            material: MaterialRole::CastArmor,
            smoothing: SmoothingGroup(2),
        })
    }

    /// The production cast turret closes both ends flat, with no artificial roof spike.
    fn dome(bumps: &[CastBump]) -> GeometryMesh {
        loft_with(CastCaps { bottom: CastCap::Planar, top: CastCap::Planar }, bumps)
    }

    /// A planar-capped loft is a closed, consistently-wound 2-manifold with finite unit normals —
    /// the one shared mesh-quality contract every generator is measured against.
    #[test]
    fn planar_capped_loft_is_a_closed_smooth_manifold() {
        let report = dome(&[])
            .validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH)
            .expect("a planar-capped cast loft is a closed smooth manifold");
        assert_eq!(report.boundary_edges, 0);
        assert_eq!(report.non_manifold_edges, 0);
    }

    /// An open loft validates as clean under `Any` topology but carries the two terminal rings as
    /// boundary edges (`n` per open end).
    #[test]
    fn open_loft_validates_as_open_but_clean() {
        let mesh = loft_with(CastCaps { bottom: CastCap::Open, top: CastCap::Open }, &[]);
        let report = mesh
            .validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH)
            .expect("an open cast loft is otherwise clean");
        assert_eq!(report.boundary_edges, 2 * SEGMENTS, "both terminal rings are open");
    }

    /// Each cap mode contributes the expected number of boundary edges: a closed end none, an open
    /// end one ring of `n`.
    #[test]
    fn each_cap_mode_has_the_expected_boundary_edge_count() {
        let count = |caps| {
            loft_with(caps, &[])
                .quality_report(vehicle_geometry::OPEN_OR_CLOSED_MESH)
                .boundary_edges
        };
        assert_eq!(count(CastCaps { bottom: CastCap::Planar, top: CastCap::Planar }), 0);
        assert_eq!(
            count(CastCaps {
                bottom: CastCap::Apex(Vec3::new(0.0, -0.1, 0.0)),
                top: CastCap::Planar
            }),
            0
        );
        assert_eq!(count(CastCaps { bottom: CastCap::Open, top: CastCap::Planar }), SEGMENTS);
        assert_eq!(count(CastCaps { bottom: CastCap::Open, top: CastCap::Open }), 2 * SEGMENTS);
    }

    /// A planar cap fans flat in the terminal station plane: it never protrudes past the bottom and
    /// top station heights, so there is no pinched roof spike.
    #[test]
    fn planar_caps_lie_in_the_terminal_station_plane() {
        let sections = dome_sections();
        let bottom_y = sections.first().unwrap().y;
        let top_y = sections.last().unwrap().y;
        let b = dome(&[]).bounds().expect("non-empty");
        assert!((b.min.y - bottom_y).abs() < 1.0e-5, "bottom cap sits in its station plane");
        assert!((b.max.y - top_y).abs() < 1.0e-5, "top cap sits in its station plane, no spike");
    }

    /// An explicit apex cap fans to a point without emitting zero-area slivers.
    #[test]
    fn apex_cap_has_no_zero_area_triangles() {
        let caps = CastCaps {
            bottom: CastCap::Apex(Vec3::new(0.0, -0.12, 0.0)),
            top: CastCap::Apex(Vec3::new(0.0, 0.68, -0.06)),
        };
        let report = loft_with(caps, &[]).quality_report(vehicle_geometry::CLOSED_SMOOTH_MESH);
        assert_eq!(report.degenerate_triangles, 0);
        assert_eq!(report.boundary_edges, 0);
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
        let bumped = dome(&[CastBump {
            azimuth: 0.0,
            az_width: 0.4,
            y: 0.2,
            y_width: 0.2,
            amount: 0.18,
            falloff_exponent: 2.0,
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
