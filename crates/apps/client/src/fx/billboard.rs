//! Billboard geometry for the FX pass: camera-facing quads for round particles and
//! velocity-stretched quads for streaks (sparks, tracers). All quads carry `uv` in `[-1, 1]`
//! so the FX shader's radial falloff softens every edge.

use glam::Vec3;
use renderer_api::FxVertex;

/// Camera basis for billboarding: unit right and up vectors spanning the view plane.
/// Falls back to world axes when the view direction degenerates (eye == target).
pub(crate) fn camera_basis(eye: Vec3, target: Vec3) -> (Vec3, Vec3) {
    let forward = (target - eye).normalize_or_zero();
    if forward == Vec3::ZERO {
        return (Vec3::X, Vec3::Y);
    }
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    if right == Vec3::ZERO {
        // Looking straight up/down: any horizontal right works.
        return (Vec3::X, Vec3::Z);
    }
    let up = right.cross(forward);
    (right, up)
}

const QUAD_UVS: [[f32; 2]; 6] =
    [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];

/// A camera-facing square quad (6 vertices) of full width `size_m` centered on `center`.
pub(crate) fn push_billboard(
    vertices: &mut Vec<FxVertex>,
    center: Vec3,
    size_m: f32,
    color: [f32; 4],
    right: Vec3,
    up: Vec3,
) {
    let half = size_m * 0.5;
    push_quad_axes(vertices, center, right * half, up * half, color);
}

/// A quad stretched along a world-space axis: `axis_half` spans half the streak length, and the
/// width axis is built perpendicular to both the streak and the view ray so the quad stays
/// visible edge-on. Used for sparks, embers, and shell tracers.
pub(crate) fn push_stretched(
    vertices: &mut Vec<FxVertex>,
    center: Vec3,
    axis_half: Vec3,
    width_half_m: f32,
    color: [f32; 4],
    eye: Vec3,
) {
    let view = center - eye;
    let side = axis_half.cross(view).normalize_or_zero();
    let side = if side == Vec3::ZERO {
        // Streak points straight at the camera: it reads as a round glow of its width.
        camera_basis(eye, center).1
    } else {
        side
    };
    push_quad_axes(vertices, center, axis_half, side * width_half_m, color);
}

fn push_quad_axes(
    vertices: &mut Vec<FxVertex>,
    center: Vec3,
    half_u: Vec3,
    half_v: Vec3,
    color: [f32; 4],
) {
    for uv in QUAD_UVS {
        let position = center + half_u * uv[0] + half_v * uv[1];
        vertices.push(FxVertex::new(position.to_array(), uv, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billboard_quads_span_the_view_plane() {
        let eye = Vec3::new(0.0, 5.0, -10.0);
        let target = Vec3::new(3.0, 1.0, 4.0);
        let (right, up) = camera_basis(eye, target);
        let forward = (target - eye).normalize();

        // The basis is orthonormal and perpendicular to the view direction.
        assert!(right.dot(forward).abs() < 1.0e-5);
        assert!(up.dot(forward).abs() < 1.0e-5);
        assert!(right.dot(up).abs() < 1.0e-5);
        assert!((right.length() - 1.0).abs() < 1.0e-5);

        let mut vertices = Vec::new();
        push_billboard(&mut vertices, Vec3::new(1.0, 2.0, 3.0), 2.0, [1.0; 4], right, up);
        assert_eq!(vertices.len(), 6);
        for vertex in &vertices {
            let offset = Vec3::from_array(vertex.position) - Vec3::new(1.0, 2.0, 3.0);
            assert!(offset.dot(forward).abs() < 1.0e-5, "vertex lies in the view plane");
        }
    }

    #[test]
    fn stretched_quad_runs_along_its_axis_and_faces_the_eye() {
        let eye = Vec3::new(0.0, 2.0, 0.0);
        let center = Vec3::new(10.0, 2.0, 10.0);
        let axis_half = Vec3::new(0.0, 0.0, 3.0); // streak along +Z
        let mut vertices = Vec::new();
        push_stretched(&mut vertices, center, axis_half, 0.25, [1.0; 4], eye);

        let max_z = vertices.iter().map(|v| v.position[2]).fold(f32::MIN, f32::max);
        let min_z = vertices.iter().map(|v| v.position[2]).fold(f32::MAX, f32::min);
        assert!((max_z - min_z - 6.0).abs() < 1.0e-4, "full streak length is 2x axis_half");

        // The width axis must not collapse: some spread exists perpendicular to the streak.
        let max_x = vertices.iter().map(|v| v.position[0]).fold(f32::MIN, f32::max);
        let min_x = vertices.iter().map(|v| v.position[0]).fold(f32::MAX, f32::min);
        let max_y = vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
        let min_y = vertices.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min);
        assert!((max_x - min_x) + (max_y - min_y) > 0.4, "width axis spans the quad");
    }

    #[test]
    fn degenerate_views_still_produce_finite_quads() {
        let mut vertices = Vec::new();
        // Streak pointing straight at the eye.
        push_stretched(
            &mut vertices,
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 2.0),
            0.5,
            [1.0; 4],
            Vec3::ZERO,
        );
        // Eye exactly on target.
        let (right, up) = camera_basis(Vec3::ONE, Vec3::ONE);
        push_billboard(&mut vertices, Vec3::ONE, 1.0, [1.0; 4], right, up);
        for vertex in &vertices {
            assert!(vertex.position.iter().all(|component| component.is_finite()));
        }
    }
}
