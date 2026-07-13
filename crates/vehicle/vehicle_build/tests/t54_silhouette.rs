//! A deterministic, renderer-free CPU-raster silhouette check for the T-54 turret. Rather than
//! committing large rendered PNG baselines as the regression oracle, this projects the turret
//! submesh orthographically from front, side and top into a small occupancy grid and locks coverage
//! and broad proportions. It catches a collapsed, hollow or wildly mis-proportioned casting without
//! pinning exact pixels.

use glam::{Vec2, Vec3};
use vehicle_build::t54_description;
use vehicle_geometry::{GeometryMesh, MaterialRole, SubmeshKind};

const GRID: usize = 64;

/// Rasterise the mesh under projection `project` into a `GRID x GRID` occupancy grid spanning the
/// projected bounds, and return the fraction of covered cells.
fn coverage(mesh: &GeometryMesh, project: impl Fn(Vec3) -> Vec2) -> f32 {
    let pts: Vec<Vec2> = mesh.vertices().iter().map(|v| project(v.position)).collect();
    let (mut min, mut max) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for p in &pts {
        min = min.min(*p);
        max = max.max(*p);
    }
    let span = (max - min).max(Vec2::splat(1.0e-4));
    let cell = span / GRID as f32;

    let mut covered = vec![false; GRID * GRID];
    for tri in mesh.indices().chunks_exact(3) {
        let a = pts[tri[0] as usize];
        let b = pts[tri[1] as usize];
        let c = pts[tri[2] as usize];
        let lo = a.min(b).min(c);
        let hi = a.max(b).max(c);
        let (gx0, gy0) = cell_of(lo, min, cell);
        let (gx1, gy1) = cell_of(hi, min, cell);
        for gy in gy0..=gy1.min(GRID - 1) {
            for gx in gx0..=gx1.min(GRID - 1) {
                let p = min + cell * Vec2::new(gx as f32 + 0.5, gy as f32 + 0.5);
                if point_in_triangle(p, a, b, c) {
                    covered[gy * GRID + gx] = true;
                }
            }
        }
    }
    covered.iter().filter(|c| **c).count() as f32 / (GRID * GRID) as f32
}

fn cell_of(p: Vec2, min: Vec2, cell: Vec2) -> (usize, usize) {
    let gx = (((p.x - min.x) / cell.x) as isize).clamp(0, GRID as isize - 1) as usize;
    let gy = (((p.y - min.y) / cell.y) as isize).clamp(0, GRID as isize - 1) as usize;
    (gx, gy)
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let cross = |u: Vec2, v: Vec2| u.x * v.y - u.y * v.x;
    let d1 = cross(b - a, p - a);
    let d2 = cross(c - b, p - b);
    let d3 = cross(a - c, p - c);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn turret() -> GeometryMesh {
    t54_description().build().submesh(SubmeshKind::Turret).expect("turret").mesh.clone()
}

#[test]
fn the_turret_silhouette_has_solid_coverage_from_every_canonical_view() {
    let mesh = turret();
    // Front (looking down -Z): X across, Y up.
    let front = coverage(&mesh, |p| Vec2::new(p.x, p.y));
    // Side (looking down -X): Z across, Y up.
    let side = coverage(&mesh, |p| Vec2::new(p.z, p.y));
    // Top (looking down -Y): X across, Z depth.
    let top = coverage(&mesh, |p| Vec2::new(p.x, p.z));

    for (name, c) in [("front", front), ("side", side), ("top", top)] {
        assert!(c > 0.35, "{name} silhouette coverage {c:.2} is too sparse — hollow or collapsed");
        assert!(c < 0.99, "{name} silhouette coverage {c:.2} fills its whole box — not a casting");
    }
}

#[test]
fn the_turret_silhouette_reads_as_a_low_wide_casting() {
    let mesh = turret();
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for vertex in mesh.vertices().iter().filter(|v| v.material == MaterialRole::CastArmor) {
        min = min.min(vertex.position);
        max = max.max(vertex.position);
    }
    let (w, d, h) = (max.x - min.x, max.z - min.z, max.y - min.y);
    // From the front and side the turret is markedly wider/deeper than tall — the flat T-54 pancake.
    assert!(w > 1.6 * h, "front silhouette must read low and wide: w {w:.2} vs h {h:.2}");
    assert!(d > 1.6 * h, "side silhouette must read low and deep: d {d:.2} vs h {h:.2}");
    // From the top it is roughly as wide as it is deep — a broad rounded plan, not a sliver.
    assert!((w / d) > 0.6 && (w / d) < 1.6, "top plan must be broad, got w/d {:.2}", w / d);
}
