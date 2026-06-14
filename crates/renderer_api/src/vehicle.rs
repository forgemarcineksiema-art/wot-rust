//! The PBR-lite vehicle vertex contract.
//!
//! Vehicles render through a separate, heavier pipeline than terrain and simple scene meshes (which
//! stay on [`crate::SceneVertex`]). A [`VehicleVertex`] carries what normal/AO mapping needs — a
//! tangent frame, texture coordinates, a material id, and a team-tint mask — without forcing that
//! cost onto every flat-shaded mesh. POD so backends can upload it zero-copy.

use glam::{Vec2, Vec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VehicleVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Tangent frame for normal mapping: `xyz` is the unit tangent, `w` the bitangent handedness
    /// (`+1`/`-1`) so the shader reconstructs `bitangent = cross(normal, tangent) * w`.
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
    /// Material role id, indexing the vehicle material set (cast vs rolled armour, rubber, steel…).
    pub material_id: u32,
    /// How much of the per-instance team tint multiplies the albedo (`0.0` none, `1.0` full).
    pub tint_mask: f32,
}

impl VehicleVertex {
    /// A vertex with its tangent left zeroed; call [`generate_tangents`] once the mesh is assembled.
    pub const fn new(
        position: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        material_id: u32,
        tint_mask: f32,
    ) -> Self {
        Self { position, normal, tangent: [0.0, 0.0, 0.0, 1.0], uv, material_id, tint_mask }
    }
}

/// Compute a per-vertex tangent frame from positions, UVs, and normals over an indexed triangle
/// mesh (Lengyel's method): accumulate each triangle's UV-aligned tangent/bitangent at its
/// vertices, then Gram-Schmidt each tangent against the vertex normal and store the handedness in
/// `tangent.w`. Degenerate UVs fall back to an arbitrary axis orthogonal to the normal, so the
/// frame is always finite and unit-length for the shader.
pub fn generate_tangents(vertices: &mut [VehicleVertex], indices: &[u32]) {
    let mut tan = vec![Vec3::ZERO; vertices.len()];
    let mut bitan = vec![Vec3::ZERO; vertices.len()];

    for tri in indices.chunks_exact(3) {
        let [i0, i1, i2] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let (p0, p1, p2) = (pos(vertices, i0), pos(vertices, i1), pos(vertices, i2));
        let (w0, w1, w2) = (uv(vertices, i0), uv(vertices, i1), uv(vertices, i2));
        let (e1, e2) = (p1 - p0, p2 - p0);
        let (d1, d2) = (w1 - w0, w2 - w0);
        let denom = d1.x * d2.y - d2.x * d1.y;
        if denom.abs() < 1.0e-12 {
            continue;
        }
        let r = 1.0 / denom;
        let t = (e1 * d2.y - e2 * d1.y) * r;
        let b = (e2 * d1.x - e1 * d2.x) * r;
        for &i in &[i0, i1, i2] {
            tan[i] += t;
            bitan[i] += b;
        }
    }

    for (vertex, (t, b)) in vertices.iter_mut().zip(tan.iter().zip(&bitan)) {
        let n = Vec3::from_array(vertex.normal);
        let mut tangent = (*t - n * n.dot(*t)).normalize_or_zero();
        if tangent == Vec3::ZERO {
            tangent = fallback_tangent(n);
        }
        let handedness = if n.cross(tangent).dot(*b) < 0.0 { -1.0 } else { 1.0 };
        vertex.tangent = [tangent.x, tangent.y, tangent.z, handedness];
    }
}

fn pos(vertices: &[VehicleVertex], i: usize) -> Vec3 {
    Vec3::from_array(vertices[i].position)
}

fn uv(vertices: &[VehicleVertex], i: usize) -> Vec2 {
    Vec2::from_array(vertices[i].uv)
}

/// Any unit vector orthogonal to `n`, for vertices whose triangles carried degenerate UVs.
fn fallback_tangent(n: Vec3) -> Vec3 {
    let axis = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    (axis - n * n.dot(axis)).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vehicle_vertex_is_pod_with_a_stable_size() {
        assert_eq!(core::mem::size_of::<VehicleVertex>(), 56);
        let v = [VehicleVertex::new([1.0, 2.0, 3.0], [0.0, 0.0, 1.0], [0.5, 0.5], 2, 1.0)];
        assert_eq!(bytemuck::cast_slice::<_, u8>(&v).len(), 56);
    }

    #[test]
    fn tangents_follow_the_uv_u_axis_on_a_flat_quad() {
        // A quad in the XY plane (normal +Z), UV's U along +X and V along +Y.
        let mut quad = vec![
            VehicleVertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0], 0, 0.0),
            VehicleVertex::new([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0], 0, 0.0),
            VehicleVertex::new([1.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0], 0, 0.0),
            VehicleVertex::new([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0], 0, 0.0),
        ];
        generate_tangents(&mut quad, &[0, 1, 2, 0, 2, 3]);

        for vertex in &quad {
            let t = Vec3::from_array([vertex.tangent[0], vertex.tangent[1], vertex.tangent[2]]);
            assert!((t - Vec3::X).length() < 1.0e-5, "tangent should follow +U (+X): {t:?}");
            assert!(t.dot(Vec3::Z).abs() < 1.0e-5, "tangent must stay orthogonal to the normal");
            assert!(
                (vertex.tangent[3] - 1.0).abs() < 1.0e-5,
                "right-handed UV gives +1 handedness"
            );
        }
    }

    #[test]
    fn degenerate_uvs_still_yield_a_finite_unit_tangent() {
        let mut tri = vec![
            VehicleVertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 0.0),
            VehicleVertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 0.0),
            VehicleVertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 0.0], 0, 0.0),
        ];
        generate_tangents(&mut tri, &[0, 1, 2]);

        for vertex in &tri {
            let t = Vec3::from_array([vertex.tangent[0], vertex.tangent[1], vertex.tangent[2]]);
            assert!(t.is_finite() && (t.length() - 1.0).abs() < 1.0e-5);
            assert!(
                t.dot(Vec3::Y).abs() < 1.0e-5,
                "fallback tangent stays orthogonal to the normal"
            );
        }
    }

    #[test]
    fn vehicle_mesh_asset_and_material_descriptor_expose_baked_vehicle_contract() {
        use crate::{VehicleMaterialDescriptor, VehicleMeshAsset};

        let mesh = VehicleMeshAsset::new(
            vec![VehicleVertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0], 1, 1.0)],
            vec![0],
        );
        let material = VehicleMaterialDescriptor::pbr_lite(
            "t54-1951",
            "albedo.png",
            "normal.png",
            "ao_roughness.png",
            Some("cavity.png"),
        );

        assert_eq!(mesh.vertex_count(), 1);
        assert_eq!(mesh.index_count(), 1);
        assert_eq!(mesh.vertices()[0].material_id, 1);
        assert_eq!(material.label(), "t54-1951");
        assert_eq!(material.albedo_texture(), "albedo.png");
        assert_eq!(material.normal_texture(), "normal.png");
        assert_eq!(material.ao_roughness_texture(), "ao_roughness.png");
        assert_eq!(material.cavity_texture(), Some("cavity.png"));
    }
}
