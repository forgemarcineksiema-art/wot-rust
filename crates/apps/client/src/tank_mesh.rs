use glam::{Mat3, Vec3};
use net::ShellSnapshot;
use renderer_api::SceneVertex;

/// One box face: four corners wound CCW as seen from outside (`edge_u × edge_v == normal`),
/// so backface culling keeps exactly the camera-facing faces.
fn quad(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    edge_u: Vec3,
    edge_v: Vec3,
    normal: Vec3,
    color: [f32; 3],
) {
    let base = vertices.len() as u32;
    let n = normal.to_array();
    for corner in [
        center - edge_u - edge_v,
        center + edge_u - edge_v,
        center + edge_u + edge_v,
        center - edge_u + edge_v,
    ] {
        vertices.push(SceneVertex::new(corner.to_array(), n, color));
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Append a box from an explicit orthonormal basis (columns: right, up, forward). `half` is
/// the half-extent along those axes; each face winds CCW outward for backface culling.
pub(crate) fn push_oriented_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    basis: Mat3,
    color: [f32; 3],
) {
    let (right, up, fwd) = (basis.x_axis, basis.y_axis, basis.z_axis);
    let (r, u, f) = (right * half.x, up * half.y, fwd * half.z);
    quad(vertices, indices, center + f, r, u, fwd, color);
    quad(vertices, indices, center - f, u, r, -fwd, color);
    quad(vertices, indices, center + r, u, f, right, color);
    quad(vertices, indices, center - r, f, u, -right, color);
    quad(vertices, indices, center + u, f, r, up, color);
    quad(vertices, indices, center - u, r, f, -up, color);
}

/// Append an oriented box (yaw around +Y, sim forward = (sin, 0, cos)). `half` is the
/// half-extent along (right, up, forward).
pub(crate) fn push_box(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    center: Vec3,
    half: Vec3,
    yaw: f32,
    color: [f32; 3],
) {
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    push_oriented_box(vertices, indices, center, half, Mat3::from_cols(right, Vec3::Y, fwd), color);
}

/// Append bright tracer cubes for in-flight shells.
pub fn append_shell_markers(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    shells: &[ShellSnapshot],
) {
    for shell in shells {
        push_box(
            vertices,
            indices,
            Vec3::from_array(shell.position),
            Vec3::splat(0.22),
            0.0,
            [1.0, 0.85, 0.35],
        );
    }
}
