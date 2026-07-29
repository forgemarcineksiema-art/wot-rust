//! Adapter from baked [`GeometryMesh`] submeshes to the PBR-lite [`VehicleVertex`] format: the
//! kernel-authored surface mapping (parametric `uv0` or triplanar), a material id per
//! [`MaterialRole`], a team-tint mask, and a generated tangent frame. This is the bridge
//! `renderer_api` and `vehicle_geometry` deliberately cannot make themselves — only the client
//! depends on both.
//!
//! Vertices stay in the submesh's local authoring space (the renderer poses them through the mount
//! chain), so both the authored UVs and the triplanar projection are stable and the texture never
//! swims as the turret traverses.

use renderer_api::{MAPPING_PARAMETRIC, MAPPING_TRIPLANAR, VehicleVertex, generate_tangents};
use vehicle_geometry::{GeometryMesh, MaterialRole, SurfaceMapping};

/// Texels-per-metre applied to the kernel's parametric UVs (the cast/sweep charts author metres).
const UV_SCALE: f32 = 0.5;

/// Convert one baked submesh into PBR-lite vehicle vertices plus its index list. Each vertex carries
/// the kernel's declared surface mapping; tangents are generated from the parametric UVs (triplanar
/// vertices fall back to an arbitrary orthogonal tangent, which their shader branch does not rely on).
pub fn vehicle_submesh_vertices(mesh: &GeometryMesh) -> (Vec<VehicleVertex>, Vec<u32>) {
    let mut vertices: Vec<VehicleVertex> = mesh
        .vertices()
        .iter()
        .map(|vertex| {
            let (uv, mapping) = match vertex.mapping {
                SurfaceMapping::ParametricUv => {
                    ([vertex.uv0.x * UV_SCALE, vertex.uv0.y * UV_SCALE], MAPPING_PARAMETRIC)
                }
                SurfaceMapping::Triplanar => ([0.0, 0.0], MAPPING_TRIPLANAR),
            };
            VehicleVertex::new(
                vertex.position.to_array(),
                vertex.normal.to_array(),
                uv,
                material_role_id(vertex.material),
                material_tint_mask(vertex.material),
            )
            .with_mapping_mode(mapping)
            .with_shade(vertex.surface_shade)
        })
        .collect();
    let indices: Vec<u32> = mesh.indices().to_vec();
    generate_tangents(&mut vertices, &indices);
    (vertices, indices)
}

/// Stable material id per role, indexing the vehicle material set in the shader.
pub fn material_role_id(material: MaterialRole) -> u32 {
    match material {
        MaterialRole::RolledArmor => 0,
        MaterialRole::CastArmor => 1,
        MaterialRole::BarrelSteel => 2,
        MaterialRole::TrackMetal => 3,
        MaterialRole::Rubber => 4,
        MaterialRole::InteriorPrimer => 5,
        MaterialRole::InteriorMachinery => 6,
        MaterialRole::Ammunition => 7,
        MaterialRole::ExposedSteel => 8,
        MaterialRole::Canvas => 9,
    }
}

/// Armour (rolled/cast) takes the team tint; barrels, tracks, and rubber stay their absolute colour
/// — the same split [`crate::color`] applies to the `SceneVertex` path.
fn material_tint_mask(material: MaterialRole) -> f32 {
    match material {
        MaterialRole::RolledArmor | MaterialRole::CastArmor => 1.0,
        MaterialRole::BarrelSteel
        | MaterialRole::TrackMetal
        | MaterialRole::Rubber
        | MaterialRole::InteriorPrimer
        | MaterialRole::InteriorMachinery
        | MaterialRole::Ammunition
        | MaterialRole::ExposedSteel
        // Fabric is issued in its own drab, not painted with the tank.
        | MaterialRole::Canvas => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::VehicleKind;
    use vehicle_geometry::{SubmeshKind, bake_vehicle};

    #[test]
    fn t54_hull_converts_to_finite_pbr_vertices_with_a_tangent_frame() {
        let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
        let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
        let (vertices, indices) = vehicle_submesh_vertices(&hull.mesh);

        assert_eq!(vertices.len(), hull.mesh.vertex_count());
        assert_eq!(indices, hull.mesh.indices());
        assert!(!vertices.is_empty());

        for vertex in &vertices {
            assert!(vertex.uv.iter().all(|c| c.is_finite()), "UVs must be finite");
            let t =
                glam::Vec3::from_array([vertex.tangent[0], vertex.tangent[1], vertex.tangent[2]]);
            assert!((t.length() - 1.0).abs() < 1.0e-3, "tangent must be unit length");
            let n = glam::Vec3::from_array(vertex.normal);
            assert!(t.dot(n).abs() < 1.0e-2, "tangent must stay orthogonal to the normal");
            assert!(vertex.material_id <= 8);
            assert!((vertex.tangent[3].abs() - 1.0).abs() < 1.0e-5, "handedness is ±1");
        }
    }

    #[test]
    fn the_kernel_surface_mapping_flows_through_to_the_vehicle_vertex() {
        use glam::{Vec2, Vec3};
        use vehicle_geometry::{GeometryVertex, SmoothingGroup};

        let parametric = GeometryVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::Y,
            MaterialRole::RolledArmor,
            SmoothingGroup(1),
        )
        .with_uv0(Vec2::new(2.0, 4.0));
        let triplanar =
            GeometryVertex::new(Vec3::ZERO, Vec3::Y, MaterialRole::CastArmor, SmoothingGroup(1));
        let mesh = GeometryMesh::new(
            vec![parametric, triplanar, parametric.with_uv0(Vec2::new(0.0, 4.0))],
            vec![0, 1, 2],
        );

        let (vertices, _) = vehicle_submesh_vertices(&mesh);
        assert_eq!(vertices[0].mapping_mode, renderer_api::MAPPING_PARAMETRIC);
        assert_eq!(
            vertices[0].uv,
            [2.0 * UV_SCALE, 4.0 * UV_SCALE],
            "parametric UV is scaled through"
        );
        assert_eq!(vertices[1].mapping_mode, renderer_api::MAPPING_TRIPLANAR);
        assert_eq!(vertices[1].uv, [0.0, 0.0], "triplanar vertices carry no authored UV");
    }

    #[test]
    fn armour_takes_the_team_tint_while_tracks_and_rubber_stay_absolute() {
        let vehicle = bake_vehicle(VehicleKind::T54_1951).expect("T-54 bakes");
        let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
        let (vertices, _) = vehicle_submesh_vertices(&hull.mesh);

        assert!(vertices.iter().any(|v| v.tint_mask == 1.0), "armour plates take the tint");
        assert!(vertices.iter().any(|v| v.tint_mask == 0.0), "tracks/wheels stay absolute");
    }
}
