//! Adapter from baked [`GeometryMesh`] submeshes to the PBR-lite [`VehicleVertex`] format: box
//! projection for stable per-submesh UVs, a material id per [`MaterialRole`], a team-tint mask, and
//! a generated tangent frame. This is the bridge `renderer_api` and `vehicle_geometry` deliberately
//! cannot make themselves — only the client depends on both.
//!
//! Vertices stay in the submesh's local authoring space (the renderer poses them through the mount
//! chain), so the box-projected UVs are stable and the texture never swims as the turret traverses.

use renderer_api::{VehicleVertex, generate_tangents};
use vehicle_geometry::{GeometryMesh, MaterialRole};

/// Texels-per-metre for the box projection. A whole tank spans a handful of UV tiles, enough for
/// panel/weld/cavity detail without the texture reading as wallpaper.
const UV_SCALE: f32 = 0.5;

/// Convert one baked submesh into PBR-lite vehicle vertices plus its index list, with a tangent
/// frame generated from the box-projected UVs.
pub fn vehicle_submesh_vertices(mesh: &GeometryMesh) -> (Vec<VehicleVertex>, Vec<u32>) {
    let mut vertices: Vec<VehicleVertex> = mesh
        .vertices()
        .iter()
        .map(|vertex| {
            VehicleVertex::new(
                vertex.position.to_array(),
                vertex.normal.to_array(),
                box_uv(vertex.position.to_array(), vertex.normal.to_array()),
                material_role_id(vertex.material),
                material_tint_mask(vertex.material),
            )
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
    }
}

/// Armour (rolled/cast) takes the team tint; barrels, tracks, and rubber stay their absolute colour
/// — the same split [`crate::color`] applies to the `SceneVertex` path.
fn material_tint_mask(material: MaterialRole) -> f32 {
    match material {
        MaterialRole::RolledArmor | MaterialRole::CastArmor => 1.0,
        MaterialRole::BarrelSteel | MaterialRole::TrackMetal | MaterialRole::Rubber => 0.0,
    }
}

/// Project onto the plane the surface normal faces most strongly (a single box-projection slot), so
/// each face gets continuous UVs without authored seams.
fn box_uv(position: [f32; 3], normal: [f32; 3]) -> [f32; 2] {
    let [x, y, z] = position;
    let [nx, ny, nz] = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
    let (u, v) = if nx >= ny && nx >= nz {
        (z, y)
    } else if ny >= nx && ny >= nz {
        (x, z)
    } else {
        (x, y)
    };
    [u * UV_SCALE, v * UV_SCALE]
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
            assert!(vertex.material_id <= 4);
            assert!((vertex.tangent[3].abs() - 1.0).abs() < 1.0e-5, "handedness is ±1");
        }
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
