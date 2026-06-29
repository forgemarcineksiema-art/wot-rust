use glam::Mat4;
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, MeshBounds, RunningGearKinematics, SubmeshKind,
    idler_unit_mesh, road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh,
    track_link_unit_mesh,
};

use crate::{MeasuredRatio, RatioKind, RatioReport, ReferencePack};

pub(crate) fn measure_baked_vehicle(
    pack: &ReferencePack,
    vehicle: &BakedVehicle,
) -> Option<RatioReport> {
    if !pack.vehicles().contains(&vehicle.kind()) {
        return None;
    }
    let hull = visual_hull_bounds(vehicle)?;
    let turret = submesh_bounds(vehicle, SubmeshKind::Turret)?;
    let gun = submesh_bounds(vehicle, SubmeshKind::Gun)?;

    Some(RatioReport::new(
        vehicle.kind(),
        pack.clone(),
        vec![
            measure(pack, RatioKind::HullLengthToWidth, extent_z(hull) / extent_x(hull))?,
            measure(pack, RatioKind::HullHeightToLength, extent_y(hull) / extent_z(hull))?,
            measure(pack, RatioKind::TurretWidthToHullWidth, extent_x(turret) / extent_x(hull))?,
            measure(pack, RatioKind::TurretHeightToHullHeight, extent_y(turret) / extent_y(hull))?,
            measure(
                pack,
                RatioKind::GunProtrusionToHullLength,
                (gun.max.z - hull.max.z).max(0.0) / extent_z(hull),
            )?,
        ],
    ))
}

fn measure(pack: &ReferencePack, kind: RatioKind, measured: f32) -> Option<MeasuredRatio> {
    let target = pack.ratio(kind)?.clone();
    Some(MeasuredRatio::new(target, measured))
}

fn submesh_bounds(vehicle: &BakedVehicle, kind: SubmeshKind) -> Option<MeshBounds> {
    vehicle.submesh(kind)?.mesh.bounds()
}

fn visual_hull_bounds(vehicle: &BakedVehicle) -> Option<MeshBounds> {
    let mut bounds = submesh_bounds(vehicle, SubmeshKind::Hull)?;
    if let Some(kin) = RunningGearKinematics::for_vehicle(vehicle.kind()) {
        bounds = bounds.union(running_gear_bounds(&kin)?);
    }
    Some(bounds)
}

fn running_gear_bounds(kin: &RunningGearKinematics) -> Option<MeshBounds> {
    let road_wheel = road_wheel_unit_mesh(kin);
    let idler = idler_unit_mesh(kin);
    let sprocket = sprocket_unit_mesh(kin);
    let link = track_link_unit_mesh(kin);
    let mut bounds: Option<MeshBounds> = None;
    for placement in running_gear_placements(kin, 0.0, 0.0) {
        let mesh = match placement.part {
            GearPart::RoadWheel => &road_wheel,
            GearPart::Idler => &idler,
            GearPart::Sprocket => &sprocket,
            GearPart::Link => &link,
        };
        include_transformed_mesh(&mut bounds, mesh, placement.transform);
    }
    bounds
}

fn include_transformed_mesh(bounds: &mut Option<MeshBounds>, mesh: &GeometryMesh, transform: Mat4) {
    for vertex in mesh.vertices() {
        let point = transform.transform_point3(vertex.position);
        match bounds {
            Some(existing) => existing.include(point),
            None => *bounds = Some(MeshBounds::from_point(point)),
        }
    }
}

fn extent_x(bounds: MeshBounds) -> f32 {
    (bounds.max.x - bounds.min.x).max(0.001)
}

fn extent_y(bounds: MeshBounds) -> f32 {
    (bounds.max.y - bounds.min.y).max(0.001)
}

fn extent_z(bounds: MeshBounds) -> f32 {
    (bounds.max.z - bounds.min.z).max(0.001)
}
