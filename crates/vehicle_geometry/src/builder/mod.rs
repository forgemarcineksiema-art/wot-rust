use glam::Vec3;

use crate::{
    Axis, GeometryMesh, GeometryVertex, MaterialRole, ProfilePoint, RevolveSpec, SmoothingGroup,
};

mod extrude;
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

    pub fn revolve(self, spec: RevolveSpec) -> Self {
        self.append_revolve(Vec3::ZERO, spec, false)
    }

    pub fn capped_revolve_at(self, origin: Vec3, spec: RevolveSpec) -> Self {
        self.append_revolve(origin, spec, true)
    }

    fn append_revolve(mut self, origin: Vec3, spec: RevolveSpec, cap_ends: bool) -> Self {
        assert!(spec.segments >= 3);
        assert!(spec.profile.len() >= 2);
        let base = self.vertices.len() as u32;
        for segment in 0..spec.segments {
            let angle = (segment as f32 / spec.segments as f32) * std::f32::consts::TAU;
            let (sin, cos) = angle.sin_cos();
            let radial = radial_for(spec.axis, cos, sin);
            for point in &spec.profile {
                let position = origin + position_for(spec.axis, radial, *point);
                self.vertices.push(GeometryVertex::new(
                    position,
                    radial,
                    spec.material,
                    spec.smoothing,
                ));
            }
        }

        let profile_len = spec.profile.len() as u32;
        for segment in 0..spec.segments as u32 {
            let next = (segment + 1) % spec.segments as u32;
            for row in 0..profile_len - 1 {
                let a = base + segment * profile_len + row;
                let b = base + next * profile_len + row;
                let c = b + 1;
                let d = a + 1;
                let face_center = [a, b, c, d]
                    .into_iter()
                    .map(|index| self.vertices[index as usize].position)
                    .fold(Vec3::ZERO, |sum, point| sum + point)
                    / 4.0;
                let axis_point = origin
                    + axis_vector(spec.axis)
                        * ((spec.profile[row as usize].offset
                            + spec.profile[row as usize + 1].offset)
                            * 0.5);
                self.push_quad_indices_outward([a, b, c, d], face_center - axis_point);
            }
        }
        if cap_ends {
            self.push_revolve_cap(origin, &spec, base, 0, -axis_vector(spec.axis));
            self.push_revolve_cap(
                origin,
                &spec,
                base,
                spec.profile.len() as u32 - 1,
                axis_vector(spec.axis),
            );
        }
        self
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

    fn push_quad(&mut self, points: [Vec3; 4], material: MaterialRole, smoothing: SmoothingGroup) {
        let base = self.vertices.len() as u32;
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        for point in points {
            self.vertices.push(GeometryVertex::new(point, normal, material, smoothing));
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn push_tri(&mut self, points: [Vec3; 3], material: MaterialRole, smoothing: SmoothingGroup) {
        let base = self.vertices.len() as u32;
        let normal = (points[1] - points[0]).cross(points[2] - points[0]).normalize_or_zero();
        for point in points {
            self.vertices.push(GeometryVertex::new(point, normal, material, smoothing));
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    fn push_quad_indices_outward(&mut self, indices: [u32; 4], outward: Vec3) {
        let [a, b, c, d] = indices;
        let normal = triangle_normal(
            self.vertices[a as usize].position,
            self.vertices[b as usize].position,
            self.vertices[c as usize].position,
        );
        if normal.dot(outward) < 0.0 {
            self.indices.extend_from_slice(&[a, c, b, a, d, c]);
        } else {
            self.indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }

    fn push_revolve_cap(
        &mut self,
        origin: Vec3,
        spec: &RevolveSpec,
        base: u32,
        row: u32,
        normal: Vec3,
    ) {
        let center = origin + axis_vector(spec.axis) * spec.profile[row as usize].offset;
        let center_index = self.vertices.len() as u32;
        self.vertices.push(GeometryVertex::new(center, normal, spec.material, spec.smoothing));
        let profile_len = spec.profile.len() as u32;
        for segment in 0..spec.segments as u32 {
            let current = base + segment * profile_len + row;
            let next = base + ((segment + 1) % spec.segments as u32) * profile_len + row;
            let face_normal = triangle_normal(
                self.vertices[center_index as usize].position,
                self.vertices[current as usize].position,
                self.vertices[next as usize].position,
            );
            if face_normal.dot(normal) < 0.0 {
                self.indices.extend_from_slice(&[center_index, next, current]);
            } else {
                self.indices.extend_from_slice(&[center_index, current, next]);
            }
        }
    }
}

fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (b - a).cross(c - a)
}

fn radial_for(axis: Axis, cos: f32, sin: f32) -> Vec3 {
    match axis {
        Axis::X => Vec3::new(0.0, cos, sin),
        Axis::Y => Vec3::new(cos, 0.0, sin),
        Axis::Z => Vec3::new(cos, sin, 0.0),
    }
}

fn position_for(axis: Axis, radial: Vec3, point: ProfilePoint) -> Vec3 {
    radial * point.radius
        + match axis {
            Axis::X => Vec3::X * point.offset,
            Axis::Y => Vec3::Y * point.offset,
            Axis::Z => Vec3::Z * point.offset,
        }
}

fn axis_vector(axis: Axis) -> Vec3 {
    match axis {
        Axis::X => Vec3::X,
        Axis::Y => Vec3::Y,
        Axis::Z => Vec3::Z,
    }
}
