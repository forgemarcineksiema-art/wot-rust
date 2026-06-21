//! Naive Surface Nets: extract a triangle mesh from a sampled [`Sdf`].
//!
//! One vertex per surface-straddling cell, placed at the average of its sign-changing edge crossings
//! (normal from the SDF gradient); quads stitched across sign-changing grid edges. Surface Nets
//! deliberately *averages*, so it reads smooth — great for a cast turret, and the honest weak point
//! for a sharp glacis edge. That tradeoff is exactly what the spike measures.

use glam::Vec3;
use sdf::Sdf;
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

/// A uniform cell grid covering `[min, min + dims * cell]`.
pub struct Grid {
    min: Vec3,
    cell: f32,
    dims: [usize; 3],
}

impl Grid {
    /// A grid wrapping `[min, max]` at roughly `target` cells along the longest axis, padded one
    /// empty cell on every side so the extracted surface stays closed.
    pub fn wrapping(min: Vec3, max: Vec3, target: usize) -> Self {
        let cell = ((max - min).max_element() / target.max(1) as f32).max(1.0e-4);
        let span = max - min;
        let dims = [
            (span.x / cell).ceil() as usize + 2,
            (span.y / cell).ceil() as usize + 2,
            (span.z / cell).ceil() as usize + 2,
        ];
        Self { min: min - Vec3::splat(cell), cell, dims }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell
    }
}

// The 12 cube edges as pairs of corner indices (corner bit 0,1,2 = x,y,z).
const CORNER_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn corner_offset(c: usize) -> Vec3 {
    Vec3::new((c & 1) as f32, ((c >> 1) & 1) as f32, ((c >> 2) & 1) as f32)
}

fn gradient(sdf: &Sdf, p: Vec3) -> Vec3 {
    let h = 1.0e-3;
    Vec3::new(
        sdf.eval(p + Vec3::X * h) - sdf.eval(p - Vec3::X * h),
        sdf.eval(p + Vec3::Y * h) - sdf.eval(p - Vec3::Y * h),
        sdf.eval(p + Vec3::Z * h) - sdf.eval(p - Vec3::Z * h),
    )
    .normalize_or_zero()
}

fn emit_quad(indices: &mut Vec<u32>, vertices: &[GeometryVertex], a: u32, b: u32, c: u32, d: u32) {
    if [a, b, c, d].contains(&u32::MAX) {
        return;
    }
    // Orient the quad so its winding agrees with the outward SDF-gradient normals at its corners.
    // Surface Nets stitches the dual quad without a fixed sense, so without this the winding is
    // effectively random; aligning it to the gradient gives a consistently outward-wound surface.
    let p = |i: u32| vertices[i as usize].position;
    let n = |i: u32| vertices[i as usize].normal;
    let face = (p(b) - p(a)).cross(p(c) - p(a));
    let outward = n(a) + n(b) + n(c) + n(d);
    if face.dot(outward) >= 0.0 {
        indices.extend_from_slice(&[a, b, c, a, c, d]);
    } else {
        indices.extend_from_slice(&[a, c, b, a, d, c]);
    }
}

/// Mesh `sdf` over `grid`, tagging every vertex with `material`/`smoothing`.
pub fn mesh_sdf(
    sdf: &Sdf,
    grid: &Grid,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> GeometryMesh {
    let [nx, ny, nz] = grid.dims;
    let (sx, sy, sz) = (nx + 1, ny + 1, nz + 1);
    let mut field = vec![0.0_f32; sx * sy * sz];
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                let p = grid.min + Vec3::new(x as f32, y as f32, z as f32) * grid.cell;
                field[x + sx * (y + sy * z)] = sdf.eval(p);
            }
        }
    }
    let s = |x: usize, y: usize, z: usize| field[x + sx * (y + sy * z)];

    let mut cell_vertex = vec![u32::MAX; nx * ny * nz];
    let mut vertices: Vec<GeometryVertex> = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let cv: [f32; 8] =
                    std::array::from_fn(|c| s(x + (c & 1), y + ((c >> 1) & 1), z + ((c >> 2) & 1)));
                let mut sum = Vec3::ZERO;
                let mut count = 0.0_f32;
                for &(a, b) in &CORNER_EDGES {
                    if (cv[a] < 0.0) != (cv[b] < 0.0) {
                        let t = cv[a] / (cv[a] - cv[b]);
                        sum += corner_offset(a).lerp(corner_offset(b), t);
                        count += 1.0;
                    }
                }
                if count == 0.0 {
                    continue;
                }
                let local = Vec3::new(x as f32, y as f32, z as f32) + sum / count;
                let world = grid.min + local * grid.cell;
                cell_vertex[x + nx * (y + ny * z)] = vertices.len() as u32;
                vertices.push(GeometryVertex::new(
                    world,
                    gradient(sdf, world),
                    material,
                    smoothing,
                ));
            }
        }
    }

    let vid = |x: usize, y: usize, z: usize| cell_vertex[x + nx * (y + ny * z)];
    let mut indices: Vec<u32> = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if vid(x, y, z) == u32::MAX {
                    continue;
                }
                let inside = s(x, y, z) < 0.0;
                if y >= 1 && z >= 1 && inside != (s(x + 1, y, z) < 0.0) {
                    emit_quad(
                        &mut indices,
                        &vertices,
                        vid(x, y, z),
                        vid(x, y - 1, z),
                        vid(x, y - 1, z - 1),
                        vid(x, y, z - 1),
                    );
                }
                if x >= 1 && z >= 1 && inside != (s(x, y + 1, z) < 0.0) {
                    emit_quad(
                        &mut indices,
                        &vertices,
                        vid(x, y, z),
                        vid(x, y, z - 1),
                        vid(x - 1, y, z - 1),
                        vid(x - 1, y, z),
                    );
                }
                if x >= 1 && y >= 1 && inside != (s(x, y, z + 1) < 0.0) {
                    emit_quad(
                        &mut indices,
                        &vertices,
                        vid(x, y, z),
                        vid(x - 1, y, z),
                        vid(x - 1, y - 1, z),
                        vid(x, y - 1, z),
                    );
                }
            }
        }
    }
    GeometryMesh::new(vertices, indices)
}

/// Mesh `sdf` at the finest uniform grid that still fits `max_tris`. Surface Nets tessellates
/// roughly uniformly, so triangle count grows about with `target^2`; we estimate the on-budget
/// resolution from one trial mesh, then step the grid coarser until it fits. This is the pivot's
/// "budget governs mesh resolution" rule made concrete.
pub fn mesh_within_budget(
    sdf: &Sdf,
    min: Vec3,
    max: Vec3,
    max_tris: usize,
    material: MaterialRole,
    smoothing: SmoothingGroup,
) -> (GeometryMesh, Grid) {
    let mut target = 48usize;
    let mut grid = Grid::wrapping(min, max, target);
    let mut mesh = mesh_sdf(sdf, &grid, material, smoothing);
    if mesh.triangle_count() > max_tris.max(1) {
        let ratio = (max_tris as f32 / mesh.triangle_count() as f32).sqrt();
        target = ((target as f32 * ratio).floor() as usize).clamp(8, 96);
        grid = Grid::wrapping(min, max, target);
        mesh = mesh_sdf(sdf, &grid, material, smoothing);
    }
    while mesh.triangle_count() > max_tris && target > 8 {
        target = (target * 9 / 10).max(8);
        grid = Grid::wrapping(min, max, target);
        mesh = mesh_sdf(sdf, &grid, material, smoothing);
    }
    (mesh, grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh_sphere(target: usize) -> GeometryMesh {
        let s = Sdf::sphere(1.0);
        let grid = Grid::wrapping(Vec3::splat(-1.3), Vec3::splat(1.3), target);
        mesh_sdf(&s, &grid, MaterialRole::CastArmor, SmoothingGroup(2))
    }

    #[test]
    fn a_closed_sphere_meshes_to_a_genus_zero_surface() {
        // For a closed manifold quad mesh, Euler's formula gives V - Q = 2 (Q = quad = tris / 2).
        // A wrong stitch would break this immediately, so it locks the connectivity.
        let mesh = mesh_sphere(24);
        assert!(mesh.triangle_count() > 0, "sphere produced no triangles");
        assert_eq!(mesh.triangle_count() % 2, 0, "quads emit two triangles each");
        // The shared contract proves the watertight, consistently-wound manifold; the Euler
        // identity below additionally pins it to genus zero.
        mesh.validate_quality(vehicle_geometry::CLOSED_SMOOTH_MESH)
            .expect("a meshed sphere is a closed smooth manifold");
        let v = mesh.vertex_count() as i64;
        let q = (mesh.triangle_count() / 2) as i64;
        assert_eq!(v - q, 2, "V - Q must be 2 for a closed genus-0 surface");
    }

    #[test]
    fn the_meshed_sphere_sits_on_its_radius() {
        let mesh = mesh_sphere(32);
        let b = mesh.bounds().expect("non-empty");
        for extent in [b.max.x, b.max.y, b.max.z, -b.min.x, -b.min.y, -b.min.z] {
            assert!((extent - 1.0).abs() < 0.1, "surface radius {extent:.3} drifts from 1.0");
        }
    }

    #[test]
    fn finer_grids_spend_more_triangles() {
        assert!(mesh_sphere(48).triangle_count() > mesh_sphere(20).triangle_count());
    }

    #[test]
    fn surface_nets_rounds_a_box_corner_inward() {
        // The known weak point: a cube's sharp corner is pulled in toward the body, so the meshed
        // extent stays *under* the true half-extent. This is the glacis concern, quantified.
        let cube = Sdf::cuboid(Vec3::splat(1.0));
        let grid = Grid::wrapping(Vec3::splat(-1.4), Vec3::splat(1.4), 20);
        let mesh = mesh_sdf(&cube, &grid, MaterialRole::RolledArmor, SmoothingGroup::hard_edges());
        let corner =
            mesh.vertices().iter().map(|vtx| vtx.position.length()).fold(0.0_f32, f32::max);
        let exact_corner = Vec3::splat(1.0).length();
        assert!(
            corner < exact_corner,
            "corner {corner:.3} should round in below {exact_corner:.3}"
        );
    }

    #[test]
    fn budget_meshing_lands_under_the_triangle_cap() {
        let (mesh, _grid) = mesh_within_budget(
            &Sdf::sphere(1.0),
            Vec3::splat(-1.3),
            Vec3::splat(1.3),
            2000,
            MaterialRole::CastArmor,
            SmoothingGroup(2),
        );
        assert!(mesh.triangle_count() > 0, "budget mesh is empty");
        assert!(mesh.triangle_count() <= 2000, "got {} tris, over budget", mesh.triangle_count());
    }

    #[test]
    fn an_empty_region_meshes_to_nothing_without_panicking() {
        // A shape entirely outside the grid: no cell straddles the surface, so the mesh is empty and
        // the budget search must not divide by a zero triangle count.
        let outside = Sdf::sphere(0.5).translate(Vec3::new(100.0, 0.0, 0.0));
        let (mesh, _) = mesh_within_budget(
            &outside,
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
            2000,
            MaterialRole::CastArmor,
            SmoothingGroup(2),
        );
        assert_eq!(mesh.triangle_count(), 0);
        assert!(mesh.bounds().is_none(), "empty mesh has no bounds");
    }
}
