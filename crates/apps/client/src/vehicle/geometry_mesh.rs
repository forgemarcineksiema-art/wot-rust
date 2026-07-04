use glam::{Mat3, Vec3};
use net::TankSnapshot;
use renderer_api::SceneVertex;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{
    GearPart, GeometryMesh, RunningGearKinematics, SubmeshKind, idler_unit_mesh,
    road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh, swing_arm_unit_mesh,
    track_link_unit_mesh,
};

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

    // Animatable running gear is no longer baked into the hull; for this static/offscreen path
    // (screenshots, geometry tests) append it at rest (phase 0) so the vehicle still reads complete.
    if let Some(kin) = RunningGearKinematics::for_vehicle(snapshot.vehicle) {
        append_running_gear(vertices, indices, &pose, &kin, hull_color);
    }
    true
}

/// Append the running gear at rest (phase 0), each unit mesh placed by the hull pose. Mirrors the
/// instanced live path so the offscreen mesh matches what the renderer draws.
fn append_running_gear(
    vertices: &mut Vec<SceneVertex>,
    indices: &mut Vec<u32>,
    pose: &VehiclePose,
    kin: &RunningGearKinematics,
    hull_color: [f32; 3],
) {
    let road_wheel = road_wheel_unit_mesh(kin);
    let swing_arm = swing_arm_unit_mesh(kin);
    let idler = idler_unit_mesh(kin);
    let sprocket = sprocket_unit_mesh(kin);
    let link = track_link_unit_mesh(kin);
    let hull_basis = pose.hull_basis();
    for placement in running_gear_placements(kin, 0.0, 0.0) {
        let mesh = match placement.part {
            GearPart::RoadWheel => &road_wheel,
            GearPart::Idler => &idler,
            GearPart::Sprocket => &sprocket,
            GearPart::Link => &link,
            GearPart::SwingArm => &swing_arm,
        };
        let normal_basis = hull_basis * Mat3::from_mat4(placement.transform);
        append_mesh(
            vertices,
            indices,
            mesh,
            normal_basis,
            |point| pose.hull_point(placement.transform.transform_point3(point)),
            hull_color,
        );
    }
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
