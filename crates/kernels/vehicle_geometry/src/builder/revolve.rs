//! Revolve (lathe) surfaces: a profile swept around an axis, with optional end caps. Split from
//! the parent builder to stay reviewable; reaches the parent's private vertex/index buffers
//! directly, like the other builder submodules.

use glam::Vec3;

use super::{MeshBuilder, axis_vector, triangle_normal};
use crate::{Axis, GeometryVertex, ProfilePoint, RevolveSpec};

impl MeshBuilder {
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
        // The cap gets its own rim vertices carrying the axial cap normal, not the side wall's radial
        // normals — sharing the wall verts shaded a flat end disc as a dome (axial→radial fan).
        let profile_len = spec.profile.len() as u32;
        let rim_base = self.vertices.len() as u32;
        let segments = spec.segments as u32;
        for segment in 0..segments {
            let position = self.vertices[(base + segment * profile_len + row) as usize].position;
            self.vertices.push(GeometryVertex::new(
                position,
                normal,
                spec.material,
                spec.smoothing,
            ));
        }
        let flip = triangle_normal(
            center,
            self.vertices[rim_base as usize].position,
            self.vertices[rim_base as usize + 1].position,
        )
        .dot(normal)
            < 0.0;
        for segment in 0..segments {
            let (current, next) = (rim_base + segment, rim_base + (segment + 1) % segments);
            let tri =
                if flip { [center_index, next, current] } else { [center_index, current, next] };
            self.indices.extend_from_slice(&tri);
        }
    }
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
