//! Backend-neutral visibility culling: a world-space AABB, a frustum extracted from this
//! project's column-major `[0, 1]`-depth view-projection matrices, and the deterministic
//! chunking that splits one static scene index buffer into spatial draw ranges. The renderer
//! draws only the chunks a pass's frustum can see — the camera pass skips the terrain behind
//! the hill, the sun-shadow passes skip everything outside their focused light box. All of it
//! is plain math on plain data, unit-testable without a GPU (`docs/render-boundary.md`).

use crate::SceneVertex;

/// Axis-aligned world-space bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    /// The empty box: grows from nothing. `min > max` marks it; `is_empty` reports it.
    pub const EMPTY: Self = Self { min: [f32::INFINITY; 3], max: [f32::NEG_INFINITY; 3] };

    pub fn is_empty(&self) -> bool {
        (0..3).any(|i| self.min[i] > self.max[i])
    }

    /// Grow to contain `point`.
    pub fn grow(&mut self, point: [f32; 3]) {
        for (i, &p) in point.iter().enumerate() {
            self.min[i] = self.min[i].min(p);
            self.max[i] = self.max[i].max(p);
        }
    }

    /// The box containing every point, or `EMPTY` for no points.
    pub fn from_points(points: impl IntoIterator<Item = [f32; 3]>) -> Self {
        let mut aabb = Self::EMPTY;
        for p in points {
            aabb.grow(p);
        }
        aabb
    }
}

/// A view frustum as six inward-facing planes (`ax + by + cz + d >= 0` inside), extracted from
/// a column-major world -> clip matrix with WebGPU's `[0, 1]` depth range (Gribb–Hartmann).
/// Works for perspective (camera) and orthographic (sun shadow box) matrices alike.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [[f32; 4]; 6],
}

impl Frustum {
    pub fn from_view_proj(view_proj: &[[f32; 4]; 4]) -> Self {
        // Row i of the matrix, assembled from the column-major storage.
        let row = |i: usize| [view_proj[0][i], view_proj[1][i], view_proj[2][i], view_proj[3][i]];
        let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
        let sub = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        Self {
            planes: [
                add(r3, r0), // left:   x >= -w
                sub(r3, r0), // right:  x <=  w
                add(r3, r1), // bottom: y >= -w
                sub(r3, r1), // top:    y <=  w
                r2,          // near:   z >= 0   (WebGPU depth range)
                sub(r3, r2), // far:    z <=  w
            ],
        }
    }

    /// Conservative frustum-vs-AABB: false only when the box is fully outside one plane, so a
    /// visible chunk is never culled (the occasional corner-case box that intersects no plane
    /// but sits outside the frustum draws harmlessly). An empty box is never visible.
    pub fn intersects_aabb(&self, aabb: &Aabb) -> bool {
        if aabb.is_empty() {
            return false;
        }
        for plane in &self.planes {
            // The box's most-positive vertex along the plane normal; if even that vertex sits
            // outside (negative side), the whole box does.
            let p = [
                if plane[0] >= 0.0 { aabb.max[0] } else { aabb.min[0] },
                if plane[1] >= 0.0 { aabb.max[1] } else { aabb.min[1] },
                if plane[2] >= 0.0 { aabb.max[2] } else { aabb.min[2] },
            ];
            if plane[0] * p[0] + plane[1] * p[1] + plane[2] * p[2] + plane[3] < 0.0 {
                return false;
            }
        }
        true
    }
}

/// One spatial draw range of a chunked static scene: `index_start .. index_start + index_count`
/// into the reordered index buffer, bounded by `aabb`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneChunk {
    pub index_start: u32,
    pub index_count: u32,
    pub aabb: Aabb,
}

/// Split a static scene mesh into spatial chunks on a regular XZ grid: triangles are bucketed
/// by centroid, indices are reordered so each chunk's triangles are contiguous, and each chunk
/// carries the exact 3D AABB of its triangles (not the flat grid cell — a triangle leaning out
/// of its centroid cell is still fully inside its chunk's box, so culling stays conservative).
///
/// Deterministic: chunks come out in row-major grid order and triangles keep their relative
/// order within a chunk, so the same mesh always chunks identically. Only opaque, depth-tested
/// geometry may be chunked — reordering draws is invisible under a depth test, and the golden
/// screenshot locks hold the renderer to exactly that.
pub fn chunk_scene_indices(
    vertices: &[SceneVertex],
    indices: &[u32],
    chunk_size_m: f32,
) -> (Vec<u32>, Vec<SceneChunk>) {
    assert!(chunk_size_m > 0.0, "chunk size must be positive");
    let triangle_count = indices.len() / 3;
    if triangle_count == 0 {
        return (indices.to_vec(), Vec::new());
    }

    // Grid extent from the referenced vertices' XZ footprint.
    let mut min_x = f32::INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for &i in indices {
        let p = vertices[i as usize].position;
        min_x = min_x.min(p[0]);
        max_x = max_x.max(p[0]);
        min_z = min_z.min(p[2]);
        max_z = max_z.max(p[2]);
    }
    let cells_x = (((max_x - min_x) / chunk_size_m).ceil() as usize).max(1);
    let cells_z = (((max_z - min_z) / chunk_size_m).ceil() as usize).max(1);

    // Bucket triangles by centroid cell; grow each cell's exact AABB by the full triangle.
    let mut cell_triangles: Vec<Vec<usize>> = vec![Vec::new(); cells_x * cells_z];
    let mut cell_aabbs: Vec<Aabb> = vec![Aabb::EMPTY; cells_x * cells_z];
    for tri in 0..triangle_count {
        let corners = [
            vertices[indices[tri * 3] as usize].position,
            vertices[indices[tri * 3 + 1] as usize].position,
            vertices[indices[tri * 3 + 2] as usize].position,
        ];
        let cx = (corners[0][0] + corners[1][0] + corners[2][0]) / 3.0;
        let cz = (corners[0][2] + corners[1][2] + corners[2][2]) / 3.0;
        let gx = (((cx - min_x) / chunk_size_m) as usize).min(cells_x - 1);
        let gz = (((cz - min_z) / chunk_size_m) as usize).min(cells_z - 1);
        let cell = gz * cells_x + gx;
        cell_triangles[cell].push(tri);
        for corner in corners {
            cell_aabbs[cell].grow(corner);
        }
    }

    // Emit non-empty cells in row-major order as contiguous index ranges.
    let mut reordered = Vec::with_capacity(indices.len());
    let mut chunks = Vec::new();
    for cell in 0..cell_triangles.len() {
        if cell_triangles[cell].is_empty() {
            continue;
        }
        let index_start = reordered.len() as u32;
        for &tri in &cell_triangles[cell] {
            reordered.extend_from_slice(&indices[tri * 3..tri * 3 + 3]);
        }
        chunks.push(SceneChunk {
            index_start,
            index_count: reordered.len() as u32 - index_start,
            aabb: cell_aabbs[cell],
        });
    }
    (reordered, chunks)
}
