use glam::Vec3;

use super::SG_MANTLET;
use crate::{GeometryMesh, GeometryVertex, MaterialRole};

pub(super) fn oval_socket_mesh(
    origin: Vec3,
    radius: f32,
    back_z: f32,
    front_z: f32,
    x_scale: f32,
    y_scale: f32,
    segments: usize,
) -> GeometryMesh {
    let span = (front_z - back_z).max(0.04);
    let profile = [
        (radius * 0.72, back_z),
        (radius * 1.08, back_z + span * 0.18),
        (radius * 1.02, back_z + span * 0.46),
        (radius * 0.86, back_z + span * 0.74),
        (radius * 0.64, front_z),
    ];
    let mut vertices = Vec::with_capacity(segments * profile.len() + 2);
    for segment in 0..segments {
        let angle = (segment as f32 / segments as f32) * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        let normal =
            Vec3::new(cos / x_scale.max(0.001), sin / y_scale.max(0.001), 0.0).normalize_or_zero();
        for (ring_radius, z) in profile {
            vertices.push(GeometryVertex::new(
                origin + Vec3::new(cos * ring_radius * x_scale, sin * ring_radius * y_scale, z),
                normal,
                MaterialRole::CastArmor,
                SG_MANTLET,
            ));
        }
    }

    let profile_len = profile.len() as u32;
    let mut indices = Vec::with_capacity(segments * (profile.len() - 1) * 6);
    for segment in 0..segments as u32 {
        let next = (segment + 1) % segments as u32;
        for row in 0..profile_len - 1 {
            let a = segment * profile_len + row;
            let b = next * profile_len + row;
            let c = b + 1;
            let d = a + 1;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    GeometryMesh::new(vertices, indices).weld_and_smooth()
}
