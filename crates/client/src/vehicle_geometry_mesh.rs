use game_core::math::rotate_around;
use glam::{Mat3, Vec3};
use net::TankSnapshot;
use renderer_api::SceneVertex;
use vehicle_geometry::{GeometryMesh, SubmeshKind, bake_vehicle};

use crate::color::{material_color, shade_color};

pub(crate) fn append_baked_tank_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> bool {
    let Ok(vehicle) = bake_vehicle(snapshot.vehicle) else {
        return false;
    };

    let ground = Vec3::from_array(snapshot.position);
    let hull_rotation = Mat3::from_rotation_y(snapshot.yaw_rad);
    let turret_rotation =
        Mat3::from_rotation_y(snapshot.vehicle.effective_turret_yaw_rad(snapshot.turret_yaw_rad));
    let gun_pitch = Mat3::from_rotation_x(-snapshot.gun_pitch_rad);
    let mounts = vehicle.mounts();

    let Some(hull) = vehicle.submesh(SubmeshKind::Hull) else {
        return false;
    };
    append_mesh(
        vertices,
        indices,
        &hull.mesh,
        hull_rotation,
        |point| ground + hull_rotation * point,
        hull_color,
    );

    let Some(turret) = vehicle.submesh(SubmeshKind::Turret) else {
        return false;
    };
    let turret_ring = mounts.turret_ring.translation;
    append_mesh(
        vertices,
        indices,
        &turret.mesh,
        hull_rotation * turret_rotation,
        |point| ground + hull_rotation * rotate_around(point, turret_ring, turret_rotation),
        hull_color,
    );

    let Some(gun) = vehicle.submesh(SubmeshKind::Gun) else {
        return false;
    };
    let trunnion = mounts.gun_trunnion.translation;
    append_mesh(
        vertices,
        indices,
        &gun.mesh,
        hull_rotation * turret_rotation * gun_pitch,
        |point| {
            let pitched = rotate_around(point, trunnion, gun_pitch);
            ground + hull_rotation * rotate_around(pitched, turret_ring, turret_rotation)
        },
        hull_color,
    );
    true
}

fn append_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    mesh: &GeometryMesh,
    normal_basis: Mat3,
    transform_point: impl Fn(Vec3) -> Vec3,
    hull_color: [f32; 3],
) {
    let base = vertices.len() as u32;
    for vertex in mesh.vertices() {
        let normal = (normal_basis * vertex.normal).normalize_or_zero();
        vertices.push(SceneVertex::new(
            transform_point(vertex.position).to_array(),
            normal.to_array(),
            shade_color(material_color(vertex.material, hull_color), vertex.surface_shade),
        ));
    }
    indices.extend(mesh.indices().iter().map(|index| base + *index));
}
