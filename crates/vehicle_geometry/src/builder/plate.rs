//! The plate operation: a box chamfered on all twelve edges — a cut-steel armour plate with a
//! beveled perimeter and hard normal seams. Where [`super::MeshBuilder::chamfered_prism`] bevels
//! only the four edges parallel to Z (flat front/back caps), `plate_box` cuts every edge, so a
//! glacis, side skirt, applique slab, or hatch cover reads as a real plate with rolled edges from
//! any angle.

use glam::Vec3;

use super::MeshBuilder;
use super::section::{oriented_quad, oriented_tri};
use crate::{MaterialRole, SmoothingGroup};

impl MeshBuilder {
    /// Append a box of half-extents `half` centred at `center`, with a uniform `bevel` chamfer cut
    /// from every edge. The plate is convex, so faces are wound outward from its centre. `bevel` is
    /// clamped so the chamfer never eats past an edge's midpoint.
    pub fn plate_box(
        mut self,
        center: Vec3,
        half: Vec3,
        bevel: f32,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) -> Self {
        let g = PlateGrid::new(center, half, bevel);

        // The six inset face rectangles: each reaches the full extent on its own axis and is pulled
        // in by the bevel on the two tangent axes.
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let quad = [
                    g.corner(axis, sign, -1.0, -1.0),
                    g.corner(axis, sign, 1.0, -1.0),
                    g.corner(axis, sign, 1.0, 1.0),
                    g.corner(axis, sign, -1.0, 1.0),
                ];
                self.push_quad(oriented_quad(quad, g.outward(&quad)), material, smoothing);
            }
        }

        // The twelve edge chamfers: each strip joins the two face rectangles that meet at an edge.
        for e in 0..3 {
            let (p, q) = ((e + 1) % 3, (e + 2) % 3);
            for sp in [-1.0, 1.0] {
                for sq in [-1.0, 1.0] {
                    let quad = [
                        g.edge(p, sp, q, sq, -1.0),
                        g.edge(q, sq, p, sp, -1.0),
                        g.edge(q, sq, p, sp, 1.0),
                        g.edge(p, sp, q, sq, 1.0),
                    ];
                    self.push_quad(oriented_quad(quad, g.outward(&quad)), material, smoothing);
                }
            }
        }

        // The eight corner triangles: each joins the three face rectangles meeting at a box corner.
        for &sx in &[-1.0, 1.0] {
            for &sy in &[-1.0, 1.0] {
                for &sz in &[-1.0, 1.0] {
                    let tri =
                        [g.corner(0, sx, sy, sz), g.corner(1, sy, sz, sx), g.corner(2, sz, sx, sy)];
                    let outward = (tri[0] + tri[1] + tri[2]) / 3.0 - g.center;
                    self.push_tri(oriented_tri(tri, outward), material, smoothing);
                }
            }
        }
        self
    }
}

/// The geometry of one chamfered plate: its centre, half-extents, and clamped bevel. Resolves the
/// inset face-corner vertices the three face groups (faces, edge strips, corners) all share.
struct PlateGrid {
    center: Vec3,
    half: [f32; 3],
    bevel: f32,
}

impl PlateGrid {
    fn new(center: Vec3, half: Vec3, bevel: f32) -> Self {
        let half = [half.x, half.y, half.z];
        let bevel = bevel.max(0.0).min(half[0] * 0.49).min(half[1] * 0.49).min(half[2] * 0.49);
        Self { center, half, bevel }
    }

    /// One inset face-corner: full extent `sign` on `axis`, pulled in by the bevel on the two
    /// tangent axes at signs `us`/`vs` (the tangents are the axes after `axis`, cyclically).
    fn corner(&self, axis: usize, sign: f32, us: f32, vs: f32) -> Vec3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let mut p = [0.0_f32; 3];
        p[axis] = sign * self.half[axis];
        p[u] = us * (self.half[u] - self.bevel);
        p[v] = vs * (self.half[v] - self.bevel);
        self.center + Vec3::from_array(p)
    }

    /// A face-corner on `axis`'s face shared with the neighbour `(naxis, nsign)`, at position `es`
    /// along the edge (whichever tangent of `axis` is not the neighbour carries the edge position).
    fn edge(&self, axis: usize, sign: f32, naxis: usize, nsign: f32, es: f32) -> Vec3 {
        let u = (axis + 1) % 3;
        let (us, vs) = if u == naxis { (nsign, es) } else { (es, nsign) };
        self.corner(axis, sign, us, vs)
    }

    fn outward(&self, quad: &[Vec3; 4]) -> Vec3 {
        (quad[0] + quad[1] + quad[2] + quad[3]) / 4.0 - self.center
    }
}
