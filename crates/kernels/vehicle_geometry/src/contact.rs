//! Renderer-free surface-contact queries against a baked submesh: a flat median-split BVH over
//! the triangles, a ray cast, and a nearest-point fallback.
//!
//! The client uses this to seat shell-impact marks flush on the *visual* armor. Gameplay resolves
//! hits on the coarse armor volumes, which sit inset from the detailed mesh, so the replicated hit
//! point floats a few centimetres proud of the surface the player sees. Casting the shell's line
//! against the real mesh — and, when the line grazes past the inset surface, snapping to the
//! nearest point — recovers the true contact point and its interpolated normal. Queries happen
//! only on damage events (a few per second) and the index is built once per vehicle kind, so the
//! BVH is about traversal simplicity and determinism, not shaving microseconds.

use glam::{Vec2, Vec3};

use crate::mesh::GeometryMesh;

/// How far a conformal patch lifts off the armor along the contact normal — enough to clear the
/// shadow-pass acne without reading as a float.
const PATCH_LIFT_M: f32 = 0.003;

/// A shell hole that WRAPS the armor: the mesh triangles under the mark, clipped to the mark's
/// square footprint and lifted a hair off the surface, with planar decal UVs. Unlike a flat quad
/// this follows a curved casting instead of hovering across its chord. Triangle soup — `positions`
/// length is a multiple of three, and `uvs` is one entry per position.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecalPatch {
    /// World-space (or decal-frame) triangle-soup positions, lifted off the surface.
    pub positions: Vec<Vec3>,
    /// Per-vertex decal-plane coordinates in `[-1, 1]`: the position projected onto the mark's
    /// axes and normalized by its radius, so the renderer can do a radial falloff.
    pub uvs: Vec<Vec2>,
}

impl DecalPatch {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

/// A resolved point on the mesh surface: where it is, which way it faces (the barycentric blend of
/// the triangle's vertex normals, so curved castings read smooth), and which triangle answered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceContact {
    pub position: Vec3,
    pub normal: Vec3,
    pub triangle_index: usize,
    pub barycentric: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    na: Vec3,
    nb: Vec3,
    nc: Vec3,
}

impl Triangle {
    fn centroid(&self) -> Vec3 {
        (self.a + self.b + self.c) / 3.0
    }

    fn min(&self) -> Vec3 {
        self.a.min(self.b).min(self.c)
    }

    fn max(&self) -> Vec3 {
        self.a.max(self.b).max(self.c)
    }

    fn geometric_normal(&self) -> Vec3 {
        (self.b - self.a).cross(self.c - self.a).normalize_or_zero()
    }

    /// Interpolate the vertex normals at barycentric `w`, falling back to the face normal when the
    /// baked vertex normals are degenerate.
    fn normal_at(&self, w: Vec3) -> Vec3 {
        let blended = (self.na * w.x + self.nb * w.y + self.nc * w.z).normalize_or_zero();
        if blended == Vec3::ZERO { self.geometric_normal() } else { blended }
    }
}

#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    fn empty() -> Self {
        Self { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) }
    }

    fn include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}

/// A leaf references `tris[start..start + count]` (via the ordering table); an internal node has
/// `count == 0` and children at `left` and `right` (a depth-first flat tree puts the right child
/// after the whole left subtree, so it cannot be inferred as `left + 1`).
#[derive(Debug, Clone, Copy)]
struct BvhNode {
    bounds: Aabb,
    left: u32,
    right: u32,
    start: u32,
    count: u32,
}

/// Triangles past this in a node stop splitting: a small leaf is cheaper to test linearly than to
/// keep subdividing, and it bounds recursion on degenerate inputs.
const LEAF_TRIANGLES: usize = 4;

#[derive(Debug, Clone)]
pub struct MeshContactIndex {
    tris: Vec<Triangle>,
    /// Triangle indices grouped so each leaf owns a contiguous span.
    order: Vec<u32>,
    nodes: Vec<BvhNode>,
}

impl MeshContactIndex {
    /// Build the index from a baked submesh, expressing every vertex in the decal-local frame by
    /// subtracting `pivot` (hull uses `Vec3::ZERO`, the turret uses its ring translation) so a
    /// query in that frame lines up with how the client stores and poses the mark.
    pub fn from_mesh(mesh: &GeometryMesh, pivot: Vec3) -> Self {
        let vertices = mesh.vertices();
        let mut tris = Vec::with_capacity(mesh.triangle_count());
        for tri in mesh.indices().chunks_exact(3) {
            let [i, j, k] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            tris.push(Triangle {
                a: vertices[i].position - pivot,
                b: vertices[j].position - pivot,
                c: vertices[k].position - pivot,
                na: vertices[i].normal,
                nb: vertices[j].normal,
                nc: vertices[k].normal,
            });
        }
        let mut order: Vec<u32> = (0..tris.len() as u32).collect();
        let mut nodes = Vec::new();
        let span = order.len();
        if !tris.is_empty() {
            build_node(&tris, &mut order, &mut nodes, 0, span);
        }
        Self { tris, order, nodes }
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// The nearest surface the ray `origin + t * dir` (for `t` in `[0, max_t]`) enters, testing
    /// front and back faces alike — the inset mesh may be approached from either side. `dir` need
    /// not be normalized; `max_t` is measured in `dir` lengths.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<SurfaceContact> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv_dir = Vec3::new(safe_inv(dir.x), safe_inv(dir.y), safe_inv(dir.z));
        let mut best: Option<(f32, usize, Vec3)> = None;
        let mut best_t = max_t;
        let mut stack = vec![0u32];
        while let Some(node_index) = stack.pop() {
            let node = self.nodes[node_index as usize];
            if !ray_hits_aabb(origin, inv_dir, node.bounds, best_t) {
                continue;
            }
            if node.count == 0 {
                stack.push(node.left);
                stack.push(node.right);
                continue;
            }
            for slot in node.start..node.start + node.count {
                let tri_index = self.order[slot as usize] as usize;
                if let Some((t, w)) = ray_triangle(origin, dir, &self.tris[tri_index])
                    && t >= 0.0
                    && t <= best_t
                {
                    best = Some((t, tri_index, w));
                    best_t = t;
                }
            }
        }
        best.map(|(t, tri_index, w)| SurfaceContact {
            position: origin + dir * t,
            normal: self.tris[tri_index].normal_at(w),
            triangle_index: tri_index,
            barycentric: w,
        })
    }

    /// The closest point on any triangle to `point`, if one lies within `max_dist`. The fallback
    /// when a grazing ray misses the inset mesh entirely. Brute force over the triangles — nearest
    /// point needs distance-to-box pruning to accelerate, and these queries are rare.
    pub fn nearest_point(&self, point: Vec3, max_dist: f32) -> Option<SurfaceContact> {
        let mut best: Option<(f32, usize, Vec3, Vec3)> = None;
        let mut best_d2 = max_dist * max_dist;
        for (tri_index, tri) in self.tris.iter().enumerate() {
            let (closest, w) = closest_point_on_triangle(point, tri);
            let d2 = closest.distance_squared(point);
            if d2 <= best_d2 {
                best_d2 = d2;
                best = Some((d2, tri_index, closest, w));
            }
        }
        best.map(|(_, tri_index, closest, w)| SurfaceContact {
            position: closest,
            normal: self.tris[tri_index].normal_at(w),
            triangle_index: tri_index,
            barycentric: w,
        })
    }

    /// Clip the mesh under a contact into a conformal decal patch: the triangles within the mark's
    /// square footprint (`radius` half-extent on each in-plane axis), clipped to that box, lifted
    /// off the surface, capped at `cap` output triangles (nearest kept). The mark then wraps a
    /// curved casting instead of hovering across its chord.
    pub fn clip_patch(&self, contact: &SurfaceContact, radius: f32, cap: usize) -> DecalPatch {
        let center = contact.position;
        let normal = contact.normal.normalize_or(Vec3::Y);
        let (u, v) = plane_basis(normal);
        // A generous gather radius so a triangle straddling the box edge is still considered.
        let gather = radius * 1.8;
        let gather_sq = gather * gather;

        // Per source triangle: its clipped fan and how close it sits, so the cap keeps the nearest.
        let mut pieces: Vec<(f32, Vec<[Vec3; 3]>)> = Vec::new();
        for tri in &self.tris {
            if tri.centroid().distance_squared(center) > gather_sq
                && tri.a.distance_squared(center) > gather_sq
                && tri.b.distance_squared(center) > gather_sq
                && tri.c.distance_squared(center) > gather_sq
            {
                continue;
            }
            let polygon = clip_to_box([tri.a, tri.b, tri.c], center, u, v, radius);
            if polygon.len() < 3 {
                continue;
            }
            let mut fan = Vec::new();
            for i in 1..polygon.len() - 1 {
                fan.push([polygon[0], polygon[i], polygon[i + 1]]);
            }
            let dist = tri.centroid().distance(center);
            pieces.push((dist, fan));
        }

        pieces.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut patch = DecalPatch::default();
        'outer: for (_, fan) in pieces {
            for tri in fan {
                if patch.triangle_count() >= cap {
                    break 'outer;
                }
                for corner in tri {
                    let lifted = corner + normal * PATCH_LIFT_M;
                    let uv = Vec2::new(
                        (corner - center).dot(u) / radius,
                        (corner - center).dot(v) / radius,
                    );
                    patch.positions.push(lifted);
                    patch.uvs.push(uv.clamp(Vec2::splat(-1.0), Vec2::splat(1.0)));
                }
            }
        }
        patch
    }

    /// A linear ray cast over every triangle, ignoring the BVH — the reference the BVH is tested
    /// against so a tree bug cannot hide behind matching-looking marks. Unconditionally public
    /// (not `cfg(test)`): the oracle is part of the crate's contract, and the production-bake
    /// comparison lives in `vehicle_recipes/tests` since the recipes moved there (W4 F4b).
    /// Never call it on a hot path — it exists to be slow and obviously correct.
    pub fn raycast_brute(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<SurfaceContact> {
        let mut best: Option<(f32, usize, Vec3)> = None;
        let mut best_t = max_t;
        for (tri_index, tri) in self.tris.iter().enumerate() {
            if let Some((t, w)) = ray_triangle(origin, dir, tri)
                && t >= 0.0
                && t <= best_t
            {
                best = Some((t, tri_index, w));
                best_t = t;
            }
        }
        best.map(|(t, tri_index, w)| SurfaceContact {
            position: origin + dir * t,
            normal: self.tris[tri_index].normal_at(w),
            triangle_index: tri_index,
            barycentric: w,
        })
    }
}

/// Recursively build a node covering `order[start..end]`, appending it and returning its index.
fn build_node(
    tris: &[Triangle],
    order: &mut [u32],
    nodes: &mut Vec<BvhNode>,
    start: usize,
    end: usize,
) -> u32 {
    let mut bounds = Aabb::empty();
    let mut centroid_bounds = Aabb::empty();
    for &tri_index in &order[start..end] {
        let tri = &tris[tri_index as usize];
        bounds.include(tri.min());
        bounds.include(tri.max());
        centroid_bounds.include(tri.centroid());
    }

    let node_index = nodes.len() as u32;
    let count = end - start;
    if count <= LEAF_TRIANGLES {
        nodes.push(BvhNode { bounds, left: 0, right: 0, start: start as u32, count: count as u32 });
        return node_index;
    }

    // Split the widest centroid axis at its midpoint; fall back to a median split if everything
    // lands on one side (coincident centroids).
    let extent = centroid_bounds.max - centroid_bounds.min;
    let axis = widest_axis(extent);
    let mid_value = 0.5 * (centroid_bounds.min[axis] + centroid_bounds.max[axis]);
    let mut mid = partition(tris, &mut order[start..end], axis, mid_value) + start;
    if mid == start || mid == end {
        mid = start + count / 2;
        order[start..end].sort_by(|&l, &r| {
            tris[l as usize].centroid()[axis]
                .partial_cmp(&tris[r as usize].centroid()[axis])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Reserve this node's slot before recursing so the children get the following indices. The
    // left root lands right after this node; the right root only after the whole left subtree, so
    // both child indices are captured, not inferred.
    nodes.push(BvhNode { bounds, left: 0, right: 0, start: 0, count: 0 });
    let left = build_node(tris, order, nodes, start, mid);
    let right = build_node(tris, order, nodes, mid, end);
    nodes[node_index as usize].left = left;
    nodes[node_index as usize].right = right;
    node_index
}

/// Sort `slice` in place so centroids below `mid_value` on `axis` come first; returns the split
/// count (index into `slice`).
fn partition(tris: &[Triangle], slice: &mut [u32], axis: usize, mid_value: f32) -> usize {
    let mut left = 0;
    for i in 0..slice.len() {
        if tris[slice[i] as usize].centroid()[axis] < mid_value {
            slice.swap(i, left);
            left += 1;
        }
    }
    left
}

/// An orthonormal in-plane basis `(u, v)` for a decal whose normal is `normal`.
fn plane_basis(normal: Vec3) -> (Vec3, Vec3) {
    let mut u = normal.cross(Vec3::Y);
    if u.length_squared() < 1.0e-6 {
        u = normal.cross(Vec3::X);
    }
    let u = u.normalize_or(Vec3::X);
    (u, normal.cross(u).normalize_or(Vec3::Z))
}

/// Sutherland–Hodgman clip of a triangle to the square prism `|dot(p-center,u)| <= radius` and
/// `|dot(p-center,v)| <= radius` (unbounded along the normal). Returns the clipped convex polygon.
fn clip_to_box(tri: [Vec3; 3], center: Vec3, u: Vec3, v: Vec3, radius: f32) -> Vec<Vec3> {
    let mut poly = tri.to_vec();
    for (axis, sign) in [(u, 1.0_f32), (u, -1.0), (v, 1.0), (v, -1.0)] {
        // Inside the half-space when `sign * dot(p-center, axis) <= radius`.
        poly = clip_half_space(&poly, |p| sign * (p - center).dot(axis) - radius);
        if poly.len() < 3 {
            return Vec::new();
        }
    }
    poly
}

/// Keep the part of a polygon where `sd(p) <= 0`, inserting edge crossings.
fn clip_half_space(poly: &[Vec3], sd: impl Fn(Vec3) -> f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = sd(a);
        let db = sd(b);
        if da <= 0.0 {
            out.push(a);
        }
        if (da <= 0.0) != (db <= 0.0) {
            let t = da / (da - db);
            out.push(a.lerp(b, t));
        }
    }
    out
}

fn widest_axis(extent: Vec3) -> usize {
    if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    }
}

fn safe_inv(value: f32) -> f32 {
    if value.abs() < 1.0e-12 {
        1.0e12_f32.copysign(if value < 0.0 { -1.0 } else { 1.0 })
    } else {
        1.0 / value
    }
}

fn ray_hits_aabb(origin: Vec3, inv_dir: Vec3, aabb: Aabb, max_t: f32) -> bool {
    // Pad the slabs a hair: a ray running exactly IN a box face (dir component 0, origin on
    // the boundary) must still enter, or a grazing hit the triangle test accepts gets pruned
    // here and the BVH disagrees with brute force on coplanar surfaces like fender tops.
    let pad = Vec3::splat(1.0e-4);
    let t0 = (aabb.min - pad - origin) * inv_dir;
    let t1 = (aabb.max + pad - origin) * inv_dir;
    let enter = t0.min(t1).max_element().max(0.0);
    let exit = t0.max(t1).min_element();
    enter <= exit && enter <= max_t
}

/// Möller–Trumbore, two-sided. Returns `(t, barycentric)` where the weights are `(a, b, c)`.
fn ray_triangle(origin: Vec3, dir: Vec3, tri: &Triangle) -> Option<(f32, Vec3)> {
    const EPS: f32 = 1.0e-7;
    let e1 = tri.b - tri.a;
    let e2 = tri.c - tri.a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < EPS {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = origin - tri.a;
    let u = tvec.dot(p) * inv_det;
    if !(-EPS..=1.0 + EPS).contains(&u) {
        return None;
    }
    let q = tvec.cross(e1);
    let v = dir.dot(q) * inv_det;
    if v < -EPS || u + v > 1.0 + EPS {
        return None;
    }
    let t = e2.dot(q) * inv_det;
    Some((t, Vec3::new(1.0 - u - v, u, v)))
}

/// Closest point on a triangle to `point`, with its barycentric weights `(a, b, c)`. The standard
/// Ericson region test (Real-Time Collision Detection).
fn closest_point_on_triangle(point: Vec3, tri: &Triangle) -> (Vec3, Vec3) {
    let ab = tri.b - tri.a;
    let ac = tri.c - tri.a;
    let ap = point - tri.a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (tri.a, Vec3::new(1.0, 0.0, 0.0));
    }
    let bp = point - tri.b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (tri.b, Vec3::new(0.0, 1.0, 0.0));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (tri.a + ab * v, Vec3::new(1.0 - v, v, 0.0));
    }
    let cp = point - tri.c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (tri.c, Vec3::new(0.0, 0.0, 1.0));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (tri.a + ac * w, Vec3::new(1.0 - w, 0.0, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (tri.b + (tri.c - tri.b) * w, Vec3::new(0.0, 1.0 - w, w));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (tri.a + ab * v + ac * w, Vec3::new(1.0 - v - w, v, w))
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::*;
    use crate::mesh::{GeometryVertex, MaterialRole, SmoothingGroup};

    fn vertex(position: Vec3, normal: Vec3) -> GeometryVertex {
        GeometryVertex::new(position, normal, MaterialRole::RolledArmor, SmoothingGroup(1))
            .with_uv0(Vec2::ZERO)
    }

    /// A unit quad facing +Z at z = 0, two triangles.
    fn quad() -> GeometryMesh {
        let n = Vec3::Z;
        let vertices = vec![
            vertex(Vec3::new(-1.0, -1.0, 0.0), n),
            vertex(Vec3::new(1.0, -1.0, 0.0), n),
            vertex(Vec3::new(1.0, 1.0, 0.0), n),
            vertex(Vec3::new(-1.0, 1.0, 0.0), n),
        ];
        GeometryMesh::new(vertices, vec![0, 1, 2, 0, 2, 3])
    }

    #[test]
    fn a_ray_finds_the_quad_and_reads_its_normal() {
        let index = MeshContactIndex::from_mesh(&quad(), Vec3::ZERO);
        let hit = index
            .raycast(Vec3::new(0.2, 0.1, 5.0), Vec3::new(0.0, 0.0, -1.0), 10.0)
            .expect("the ray meets the quad");
        assert!(hit.position.distance(Vec3::new(0.2, 0.1, 0.0)) < 1.0e-5);
        assert!(hit.normal.distance(Vec3::Z) < 1.0e-5);
    }

    #[test]
    fn a_ray_that_misses_returns_nothing() {
        let index = MeshContactIndex::from_mesh(&quad(), Vec3::ZERO);
        assert!(index.raycast(Vec3::new(5.0, 5.0, 5.0), Vec3::NEG_Z, 10.0).is_none());
    }

    #[test]
    fn nearest_point_snaps_a_grazing_query_onto_the_surface() {
        let index = MeshContactIndex::from_mesh(&quad(), Vec3::ZERO);
        let contact =
            index.nearest_point(Vec3::new(0.3, -0.2, 0.4), 1.0).expect("within reach of the quad");
        assert!(contact.position.distance(Vec3::new(0.3, -0.2, 0.0)) < 1.0e-5);
    }

    #[test]
    fn an_empty_mesh_answers_no_contact() {
        let index = MeshContactIndex::from_mesh(&GeometryMesh::default(), Vec3::ZERO);
        assert!(index.raycast(Vec3::ZERO, Vec3::NEG_Z, 10.0).is_none());
        assert!(index.nearest_point(Vec3::ZERO, 10.0).is_none());
    }

    #[test]
    fn a_clipped_patch_stays_on_the_surface_and_inside_its_footprint() {
        let index = MeshContactIndex::from_mesh(&quad(), Vec3::ZERO);
        let contact =
            index.raycast(Vec3::new(0.1, 0.1, 5.0), Vec3::NEG_Z, 10.0).expect("ray meets the quad");
        let patch = index.clip_patch(&contact, 0.3, 64);
        assert!(!patch.is_empty(), "a hit on the quad yields a patch");
        for (position, uv) in patch.positions.iter().zip(&patch.uvs) {
            // On the surface (z ~ 0) plus the tiny lift, and inside the [-1,1] decal box.
            let surface_gap =
                index.nearest_point(*position, 1.0).map(|c| c.position.distance(*position));
            assert!(
                surface_gap.is_some_and(|d| d < 0.004),
                "every patch vertex hugs the mesh, got {surface_gap:?}"
            );
            assert!(uv.x.abs() <= 1.0 + 1.0e-4 && uv.y.abs() <= 1.0 + 1.0e-4, "uv in box: {uv:?}");
        }
    }
}
