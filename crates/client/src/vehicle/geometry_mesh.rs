use glam::{Mat3, Vec3};
use net::TankSnapshot;
use renderer_api::SceneVertex;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{GeometryMesh, SubmeshKind};

use super::pose::VehiclePose;
use crate::color::{material_color, shade_color};

pub(crate) fn append_baked_tank_mesh(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    snapshot: &TankSnapshot,
    hull_color: [f32; 3],
) -> bool {
    let Ok(vehicle) = authoritative_baked_vehicle(snapshot.vehicle) else {
        return false;
    };
    let pose = VehiclePose::from_snapshot(snapshot);

    let Some(hull) = vehicle.submesh(SubmeshKind::Hull) else {
        return false;
    };
    append_mesh(
        vertices,
        indices,
        &hull.mesh,
        pose.hull_basis(),
        |point| pose.hull_point(point),
        hull_color,
    );

    let Some(turret) = vehicle.submesh(SubmeshKind::Turret) else {
        return false;
    };
    append_mesh(
        vertices,
        indices,
        &turret.mesh,
        pose.turret_basis(),
        |point| pose.turret_point(point),
        hull_color,
    );

    let Some(gun) = vehicle.submesh(SubmeshKind::Gun) else {
        return false;
    };
    append_mesh(
        vertices,
        indices,
        &gun.mesh,
        pose.gun_basis(),
        |point| pose.gun_point(point),
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
