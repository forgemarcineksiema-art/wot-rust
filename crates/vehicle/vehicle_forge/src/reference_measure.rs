use glam::Mat4;
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, MaterialRole, MeshBounds, RunningGearKinematics,
    SubmeshKind, idler_unit_mesh, road_wheel_unit_mesh, running_gear_placements,
    sprocket_unit_mesh, track_link_unit_mesh,
};

use crate::{
    DimensionKind, DimensionReport, MeasuredDimension, MeasuredRatio, RatioKind, RatioReport,
    ReferencePack,
};

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

    // Every authored target is measured — a pack declares any subset of the ratio family,
    // so new kinds can arrive without forcing every pack to adopt them at once.
    let value = |kind: RatioKind| -> f32 {
        match kind {
            RatioKind::HullLengthToWidth => extent_z(hull) / extent_x(hull),
            RatioKind::HullHeightToLength => extent_y(hull) / extent_z(hull),
            RatioKind::TurretWidthToHullWidth => extent_x(turret) / extent_x(hull),
            RatioKind::TurretHeightToHullHeight => extent_y(turret) / extent_y(hull),
            RatioKind::GunProtrusionToHullLength => {
                (gun.max.z - hull.max.z).max(0.0) / extent_z(hull)
            }
            RatioKind::TurretLengthToWidth => extent_z(turret) / extent_x(turret).max(0.001),
            RatioKind::TurretRingPositionOnHull => {
                (vehicle.mounts().turret_ring.translation.z - hull.min.z) / extent_z(hull)
            }
            RatioKind::RoadWheelDiameterToHullLength => {
                RunningGearKinematics::for_vehicle(vehicle.kind())
                    .map_or(0.0, |kin| 2.0 * kin.wheel_radius / extent_z(hull))
            }
        }
    };
    let measurements = pack
        .ratios()
        .iter()
        .map(|target| MeasuredRatio::new(target.clone(), value(target.kind())))
        .collect();
    Some(RatioReport::new(vehicle.kind(), pack.clone(), measurements))
}

/// Measure the pack's absolute anchors (metres) against the baked mesh. Local space puts the
/// origin on the ground plane, so `max.y` IS height above ground — no offset juggling.
pub(crate) fn measure_dimensions(
    pack: &ReferencePack,
    vehicle: &BakedVehicle,
) -> Option<DimensionReport> {
    if !pack.vehicles().contains(&vehicle.kind()) {
        return None;
    }
    let hull = visual_hull_bounds(vehicle)?;
    let turret = submesh_bounds(vehicle, SubmeshKind::Turret)?;
    let gun = submesh_bounds(vehicle, SubmeshKind::Gun)?;

    let mut measurements = Vec::with_capacity(pack.dimensions().len());
    for target in pack.dimensions() {
        let measured_m = match target.kind() {
            DimensionKind::HullLength => extent_z(hull),
            DimensionKind::HullWidth => extent_x(hull),
            DimensionKind::HeightToTurretRoof => hull.max.y.max(turret.max.y),
            DimensionKind::OverallLengthWithGun => gun.max.z.max(hull.max.z) - hull.min.z,
            DimensionKind::RoadWheelDiameter => {
                2.0 * RunningGearKinematics::for_vehicle(vehicle.kind())?.wheel_radius
            }
        };
        measurements.push(MeasuredDimension::new(target.clone(), measured_m));
    }
    Some(DimensionReport::new(vehicle.kind(), measurements))
}

fn submesh_bounds(vehicle: &BakedVehicle, kind: SubmeshKind) -> Option<MeshBounds> {
    exterior_bounds(&vehicle.submesh(kind)?.mesh)
}

fn exterior_bounds(mesh: &GeometryMesh) -> Option<MeshBounds> {
    let mut bounds: Option<MeshBounds> = None;
    for vertex in mesh.vertices().iter().filter(|vertex| is_exterior(vertex.material)) {
        match &mut bounds {
            Some(existing) => existing.include(vertex.position),
            None => bounds = Some(MeshBounds::from_point(vertex.position)),
        }
    }
    bounds
}

fn is_exterior(material: MaterialRole) -> bool {
    !matches!(
        material,
        MaterialRole::InteriorPrimer | MaterialRole::InteriorMachinery | MaterialRole::Ammunition
    )
}

fn visual_hull_bounds(vehicle: &BakedVehicle) -> Option<MeshBounds> {
    let mut bounds = submesh_bounds(vehicle, SubmeshKind::Hull)?;
    if let Some(kin) = RunningGearKinematics::for_vehicle(vehicle.kind()) {
        bounds = bounds.union(running_gear_bounds(&kin)?);
    }
    Some(bounds)
}

/// Bounds of what a production review actually draws: all baked submeshes plus the rest-pose
/// runtime running gear. Semantic part bounds for tracks and wheels must be checked against this
/// composed envelope, not against the static hull bake alone.
pub fn composed_visual_bounds(vehicle: &BakedVehicle) -> Option<MeshBounds> {
    let mut bounds: Option<MeshBounds> = None;
    for submesh in vehicle.submeshes() {
        if let Some(submesh_bounds) = submesh.mesh.bounds() {
            bounds = Some(match bounds {
                Some(existing) => existing.union(submesh_bounds),
                None => submesh_bounds,
            });
        }
    }
    if let Some(kin) = RunningGearKinematics::for_vehicle(vehicle.kind())
        && let Some(gear_bounds) = running_gear_bounds(&kin)
    {
        bounds = Some(match bounds {
            Some(existing) => existing.union(gear_bounds),
            None => gear_bounds,
        });
    }
    bounds
}

fn running_gear_bounds(kin: &RunningGearKinematics) -> Option<MeshBounds> {
    let road_wheel = road_wheel_unit_mesh(kin);
    let idler = idler_unit_mesh(kin);
    let sprocket = sprocket_unit_mesh(kin);
    let link = track_link_unit_mesh(kin);
    let swing_arm = vehicle_geometry::swing_arm_unit_mesh(kin);
    let return_roller = vehicle_geometry::return_roller_unit_mesh(kin);
    let mut bounds: Option<MeshBounds> = None;
    for placement in running_gear_placements(kin, 0.0, 0.0) {
        let mesh = match placement.part {
            GearPart::RoadWheel => &road_wheel,
            GearPart::Idler => &idler,
            GearPart::Sprocket => &sprocket,
            GearPart::Link => &link,
            GearPart::SwingArm => &swing_arm,
            GearPart::ReturnRoller => &return_roller,
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
