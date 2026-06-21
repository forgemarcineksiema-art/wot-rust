//! A convex solid as an intersection of half-spaces, triangulated to an *exact* boundary mesh.
//!
//! This is the CAD/B-rep path in miniature: corners are exact triple-plane intersections, every face
//! is flat with the plane's true normal, so edges are razor-sharp and the armour slope is the face
//! normal itself — no meshing softens it. Convex-only (no fillets), which is all a plate block needs.

use glam::{Mat3, Vec3};
use vehicle_geometry::{GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

pub use crate::validate::ConvexSolidError;
use crate::validate::{MIN_NORMAL, is_bounded, validate_planes};

const EPS: f32 = 1.0e-4;

/// A bounding plane: the solid lies on the `dot(p, normal) <= offset` side.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub offset: f32,
}

impl Plane {
    /// `normal` need not be unit — it is normalised (and `offset` scaled to match), so the stored
    /// normal is exactly the surface slope. Infallible builder for known-good static plate data;
    /// validated construction goes through [`Plane::try_new`].
    pub fn new(normal: Vec3, offset: f32) -> Self {
        let len = normal.length().max(1.0e-9);
        Self { normal: normal / len, offset: offset / len }
    }

    /// Validated plane construction: rejects non-finite data and near-zero normals.
    pub fn try_new(normal: Vec3, offset: f32) -> Result<Self, ConvexSolidError> {
        if !normal.is_finite() || !offset.is_finite() {
            return Err(ConvexSolidError::NonFinitePlane { index: 0 });
        }
        let len = normal.length();
        if len < MIN_NORMAL {
            return Err(ConvexSolidError::DegenerateNormal { index: 0 });
        }
        Ok(Self { normal: normal / len, offset: offset / len })
    }
}

/// A convex solid defined by its bounding half-spaces.
#[derive(Debug, Clone)]
pub struct ConvexSolid {
    planes: Vec<Plane>,
}

impl ConvexSolid {
    pub fn new(planes: Vec<Plane>) -> Self {
        Self { planes }
    }

    /// Validated construction: every plane must be finite with a usable normal, and at least four
    /// distinct planes are needed to bound a volume.
    pub fn try_new(planes: Vec<Plane>) -> Result<Self, ConvexSolidError> {
        validate_planes(&planes)?;
        Ok(Self { planes })
    }

    /// Distinct planes, collapsing exact duplicates so a repeated half-space does not double-face.
    fn usable_planes(&self) -> Vec<Plane> {
        let mut out: Vec<Plane> = Vec::with_capacity(self.planes.len());
        for plane in &self.planes {
            let dup = out.iter().any(|q| {
                q.normal.distance(plane.normal) < EPS && (q.offset - plane.offset).abs() < EPS
            });
            if !dup {
                out.push(*plane);
            }
        }
        out
    }

    /// An axis-aligned box centred at `centre` with the given half-extents.
    pub fn box_at(centre: Vec3, half: Vec3) -> Self {
        let mut planes = Vec::with_capacity(6);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            planes.push(Plane::new(axis, (centre + half * axis).dot(axis)));
            planes.push(Plane::new(-axis, (centre - half * axis).dot(-axis)));
        }
        Self { planes }
    }

    /// Add a clipping half-space (e.g. the glacis plate) and return the narrowed solid.
    pub fn clipped_by(mut self, plane: Plane) -> Self {
        self.planes.push(plane);
        self
    }

    /// Consume the solid and return its bounding planes.
    pub fn into_planes(self) -> Vec<Plane> {
        self.planes
    }

    fn contains(&self, p: Vec3) -> bool {
        self.planes.iter().all(|pl| pl.normal.dot(p) <= pl.offset + EPS)
    }

    /// Every exact corner: a triple-plane intersection that satisfies every half-space.
    fn corners(&self) -> Vec<Vec3> {
        let n = self.planes.len();
        let mut pts: Vec<Vec3> = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                for k in (j + 1)..n {
                    if let Some(p) = intersect3(&self.planes[i], &self.planes[j], &self.planes[k])
                        && self.contains(p)
                        && !pts.iter().any(|q| q.distance(p) < EPS)
                    {
                        pts.push(p);
                    }
                }
            }
        }
        pts
    }

    /// Triangulate the boundary: one flat, fan-triangulated polygon per plane. Validates the
    /// half-spaces first, so it never silently emits undefined geometry from bad input.
    pub fn to_mesh(
        &self,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) -> Result<GeometryMesh, ConvexSolidError> {
        let planes = self.usable_planes();
        validate_planes(&planes)?;
        if !is_bounded(&planes) {
            return Err(ConvexSolidError::Unbounded);
        }
        let solid = ConvexSolid { planes };
        let corners = solid.corners();
        if corners.is_empty() {
            return Err(ConvexSolidError::EmptyIntersection);
        }

        let mut vertices: Vec<GeometryVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for plane in &solid.planes {
            let mut ring: Vec<Vec3> = corners
                .iter()
                .copied()
                .filter(|p| (plane.normal.dot(*p) - plane.offset).abs() < EPS)
                .collect();
            // A fully redundant plane touches no corner (skip it); a plane that touches only an edge
            // or vertex is a degenerate sliver face, which is malformed input.
            match ring.len() {
                0 => continue,
                1 | 2 => return Err(ConvexSolidError::DegenerateFace),
                _ => {}
            }
            order_ccw(&mut ring, plane.normal);
            let base = vertices.len() as u32;
            for p in &ring {
                vertices.push(GeometryVertex::new(*p, plane.normal, material, smoothing));
            }
            for k in 1..(ring.len() as u32 - 1) {
                indices.extend_from_slice(&[base, base + k, base + k + 1]);
            }
        }
        Ok(GeometryMesh::new(vertices, indices))
    }
}

fn intersect3(a: &Plane, b: &Plane, c: &Plane) -> Option<Vec3> {
    let m = Mat3::from_cols(a.normal, b.normal, c.normal).transpose();
    if m.determinant().abs() < 1.0e-7 {
        return None;
    }
    Some(m.inverse() * Vec3::new(a.offset, b.offset, c.offset))
}

/// Sort a coplanar ring counter-clockwise about `normal` (winding is irrelevant to the flat-shaded
/// preview, but a consistent fan needs an angular order so faces stay non-self-intersecting).
fn order_ccw(ring: &mut [Vec3], normal: Vec3) {
    let centroid = ring.iter().fold(Vec3::ZERO, |a, &p| a + p) / ring.len() as f32;
    let u = pick_tangent(normal);
    let w = normal.cross(u);
    ring.sort_by(|a, b| angle(*a - centroid, u, w).total_cmp(&angle(*b - centroid, u, w)));
}

fn angle(d: Vec3, u: Vec3, w: Vec3) -> f32 {
    w.dot(d).atan2(u.dot(d))
}

fn pick_tangent(n: Vec3) -> Vec3 {
    let seed = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    seed.cross(n).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_box_has_eight_corners_and_twelve_triangles() {
        let solid = ConvexSolid::box_at(Vec3::ZERO, Vec3::splat(1.0));
        assert_eq!(solid.corners().len(), 8, "a box has 8 corners");
        let mesh = solid
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("a box is a valid bounded solid");
        assert_eq!(mesh.triangle_count(), 12, "6 quad faces fan to 12 triangles");
        mesh.validate_quality(vehicle_geometry::OPEN_OR_CLOSED_MESH)
            .expect("a fabricated armour box is finite and non-degenerate");
        let b = mesh.bounds().expect("non-empty");
        assert!((b.max - Vec3::splat(1.0)).length() < EPS, "box reaches its exact extent");
    }

    #[test]
    fn clipping_a_box_keeps_exact_face_normals() {
        // A box cut by a 60-degrees-from-horizontal plane: the cut face's vertex normals equal the
        // exact slope normal — razor sharp, and the armour angle is recoverable to the bit.
        let slope = 60.0_f32.to_radians();
        let cut = Plane::new(Vec3::new(0.0, slope.cos(), slope.sin()), 1.0);
        let solid = ConvexSolid::box_at(Vec3::new(0.0, 0.6, 0.0), Vec3::splat(1.0)).clipped_by(cut);
        let mesh = solid
            .to_mesh(MaterialRole::RolledArmor, SmoothingGroup::hard_edges())
            .expect("a clipped box is a valid bounded solid");
        let has_exact_slope =
            mesh.vertices().iter().any(|v| (v.normal - cut.normal).length() < 1.0e-3);
        assert!(has_exact_slope, "the glacis face carries the exact plate normal");
        assert!(mesh.triangle_count() < 40, "a clipped box stays tiny: {}", mesh.triangle_count());
    }
}
