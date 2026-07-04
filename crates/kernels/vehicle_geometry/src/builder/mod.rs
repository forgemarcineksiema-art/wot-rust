use glam::Vec3;

use crate::{Axis, GeometryMesh, GeometryVertex, MaterialRole, SmoothingGroup};

mod extrude;
mod loft;
mod plate;
mod polygon_extrude;
mod revolve;
mod section;
mod transform;

#[derive(Debug, Default, Clone)]
pub struct MeshBuilder {
    vertices: Vec<GeometryVertex>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn chamfered_prism(
        mut self,
        center: Vec3,
        half: Vec3,
        chamfer: f32,
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) -> Self {
        let c = chamfer.min(half.x * 0.45).min(half.y * 0.45).max(0.0);
        let ring = [
            Vec3::new(-half.x + c, -half.y, -half.z),
            Vec3::new(half.x - c, -half.y, -half.z),
            Vec3::new(half.x, -half.y + c, -half.z),
            Vec3::new(half.x, half.y - c, -half.z),
            Vec3::new(half.x - c, half.y, -half.z),
            Vec3::new(-half.x + c, half.y, -half.z),
            Vec3::new(-half.x, half.y - c, -half.z),
            Vec3::new(-half.x, -half.y + c, -half.z),
        ];
        let front: Vec<Vec3> =
            ring.iter().map(|p| center + *p + Vec3::Z * (2.0 * half.z)).collect();
        let back: Vec<Vec3> = ring.iter().map(|p| center + *p).collect();

        for i in 0..back.len() {
            let j = (i + 1) % back.len();
            self.push_quad([back[i], back[j], front[j], front[i]], material, smoothing);
        }
        let back_center = center - Vec3::Z * half.z;
        let front_center = center + Vec3::Z * half.z;
        for i in 0..back.len() {
            let j = (i + 1) % back.len();
            self.push_tri([back_center, back[j], back[i]], material, smoothing);
            self.push_tri([front_center, front[i], front[j]], material, smoothing);
        }
        self
    }

    pub fn build(self) -> GeometryMesh {
        GeometryMesh::new(self.vertices, self.indices)
    }

    /// One explicit quad (CCW as seen from its outward normal). `pub(crate)` for bespoke plate
    /// authoring in recipes — faceted armor like the IS-3 pike bow is built face by face on the
    /// exact planes the armor volumes shoot against.
    pub(crate) fn push_quad(
        &mut self,
        points: [Vec3; 4],
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) {
        let base = self.vertices.len() as u32;
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        for point in points {
            self.vertices.push(GeometryVertex::new(point, normal, material, smoothing));
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// One explicit triangle (CCW from its outward normal); see [`Self::push_quad`].
    pub(crate) fn push_tri(
        &mut self,
        points: [Vec3; 3],
        material: MaterialRole,
        smoothing: SmoothingGroup,
    ) {
        let base = self.vertices.len() as u32;
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        for point in points {
            self.vertices.push(GeometryVertex::new(point, normal, material, smoothing));
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (b - a).cross(c - a)
}

fn axis_vector(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::X,
        Axis::Y => Vec3::Y,
        Axis::Z => Vec3::Z,
    }
}
