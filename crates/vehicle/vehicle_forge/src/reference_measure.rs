use game_core::VehicleBlueprint;
use glam::Mat4;
use vehicle_geometry::{
    BakedVehicle, GearPart, GeometryMesh, MaterialRole, MeshBounds, RunningGearKinematics,
    SubmeshKind, idler_unit_mesh, road_wheel_unit_mesh, running_gear_placements,
    sprocket_unit_mesh, track_link_unit_mesh,
};

use crate::{
    DimensionKind, DimensionReport, MeasuredDimension, MeasuredRatio, MeasurementBasis, RatioKind,
    RatioReport, ReferencePack,
};

pub(crate) fn measure_baked_vehicle(
    pack: &ReferencePack,
    vehicle: &BakedVehicle,
    blueprint: Option<&VehicleBlueprint>,
) -> Option<RatioReport> {
    if !pack.vehicles().contains(&vehicle.kind()) {
        return None;
    }
    let kin = gear_kinematics(vehicle, blueprint);
    // The bounds the blessed ratio targets were calibrated against — EXCEPT on the two axes the
    // catalogued hitbox exceptions contaminate, which are clamped to the armour skin: the hull's
    // LENGTH (the BDSh-5 drums added 0.36 m of stowage the documented proportions never counted)
    // and the turret's TOP (the DShK at fighting height is a gun, not the casting). Everything
    // else keeps its original instrument so the calibration of every previously-blessed target
    // survives untouched.
    let mut hull = visual_hull_bounds(vehicle, kin.as_ref())?;
    if let Some(armour) = armour_skin_bounds(vehicle, SubmeshKind::Hull) {
        hull.min.z = armour.min.z;
        hull.max.z = armour.max.z;
    }
    let mut turret = submesh_bounds(vehicle, SubmeshKind::Turret)?;
    if let Some(armour) = armour_skin_bounds(vehicle, SubmeshKind::Turret) {
        turret.max.y = turret.max.y.min(armour.max.y);
    }
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
            RatioKind::RoadWheelDiameterToHullLength => kin
                .as_ref()
                .and_then(road_wheel_diameter_from_mesh)
                .map_or(0.0, |diameter| diameter / extent_z(hull)),
        }
    };
    let measurements = pack
        .ratios()
        .iter()
        .map(|target| MeasuredRatio::new(target.clone(), value(target.kind())))
        .collect();
    Some(RatioReport::new(vehicle.kind(), pack.clone(), measurements))
}

/// Fore-aft extent of the hull's ROLLED ARMOUR — the plates, without the stowage that rides on
/// them. Returns `None` when a vehicle has no rolled plate in its hull submesh at all.
/// Bounds over a submesh's ARMOUR SKIN only — the structure the documented silhouette numbers
/// describe, with steel furniture (the catalogued protrusions among it) excluded.
fn armour_skin_bounds(
    vehicle: &vehicle_geometry::BakedVehicle,
    kind: SubmeshKind,
) -> Option<vehicle_geometry::MeshBounds> {
    let mesh = &vehicle.submesh(kind)?.mesh;
    let mut bounds: Option<vehicle_geometry::MeshBounds> = None;
    for vertex in mesh.vertices().iter().filter(|v| is_armor_skin(v.material)) {
        bounds = Some(match bounds {
            None => vehicle_geometry::MeshBounds::from_point(vertex.position),
            Some(mut acc) => {
                acc.include(vertex.position);
                acc
            }
        });
    }
    bounds
}

fn hull_mesh(vehicle: &vehicle_geometry::BakedVehicle) -> Option<&vehicle_geometry::GeometryMesh> {
    vehicle.submesh(vehicle_geometry::SubmeshKind::Hull).map(|s| &s.mesh)
}

fn armour_plate_extent_z(vehicle: &vehicle_geometry::BakedVehicle) -> Option<f32> {
    let mesh = &vehicle.submesh(vehicle_geometry::SubmeshKind::Hull)?.mesh;
    let plate: Vec<f32> = mesh
        .vertices()
        .iter()
        .filter(|v| v.material == vehicle_geometry::MaterialRole::RolledArmor)
        .map(|v| v.position.z)
        .collect();
    if plate.is_empty() {
        return None;
    }
    let lo = plate.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = plate.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    Some(hi - lo)
}

/// Measure the pack's absolute anchors (metres) against the baked mesh. Local space puts the
/// origin on the ground plane, so `max.y` IS height above ground — no offset juggling.
///
/// A measurement that cannot be produced records `NaN` and therefore FAILS its row loudly —
/// a broken instrument must never make a report row quietly disappear.
pub(crate) fn measure_dimensions(
    pack: &ReferencePack,
    vehicle: &BakedVehicle,
    live: Option<&VehicleBlueprint>,
) -> Option<DimensionReport> {
    if !pack.vehicles().contains(&vehicle.kind()) {
        return None;
    }
    let kin = gear_kinematics(vehicle, live);
    let hull = visual_hull_bounds(vehicle, kin.as_ref())?;
    let _turret = submesh_bounds(vehicle, SubmeshKind::Turret)?;
    let gun = submesh_bounds(vehicle, SubmeshKind::Gun)?;
    // A live override measures against the blueprint the author is editing; without one the
    // embedded blueprint is the truth.
    let blueprint = live.cloned().or_else(|| VehicleBlueprint::for_vehicle(vehicle.kind()));

    let mut measurements = Vec::with_capacity(pack.dimensions().len());
    for target in pack.dimensions() {
        let (measured_m, basis) = match target.kind() {
            // Over the ARMOUR PLATE, not over the kit strapped to it. The hull submesh also
            // carries the stowed unditching beam, which hangs a hand's width past the rear plate
            // — a log lashed to the stern is field equipment, and no source counts it in a hull
            // length. Measuring the rolled-armour material answers the question the dossier is
            // actually asking.
            DimensionKind::HullLength => (armour_plate_extent_z(vehicle), MeasurementBasis::Mesh),
            DimensionKind::HullWidth => (Some(extent_x(hull)), MeasurementBasis::Mesh),
            DimensionKind::HeightToTurretRoof => {
                // The documented apex is a STRUCTURE height — roof plus exposed cupola, by the
                // dossier's own derivation — so it is measured over the armour skin. The turret
                // submesh also carries the DShK at its fighting height (a catalogued hitbox
                // exception), and documented vehicle heights never include the AA gun; read off
                // raw bounds this anchor failed by exactly the machine gun.
                let apex = vehicle
                    .submesh(SubmeshKind::Turret)
                    .map(|sub| {
                        sub.mesh
                            .vertices()
                            .iter()
                            .filter(|v| is_armor_skin(v.material))
                            .map(|v| v.position.y)
                            .fold(f32::NEG_INFINITY, f32::max)
                    })
                    .filter(|apex| apex.is_finite());
                (apex.map(|apex| apex.max(hull.max.y)), MeasurementBasis::Mesh)
            }
            DimensionKind::OverallLengthWithGun => {
                // Muzzle to the ARMOUR rear. Documented overall lengths never include stowage —
                // and the hull submesh's rearmost metal is now the BDSh-5 smoke drums, 0.36 m of
                // catalogued stowage behind the plates. Measured off the whole bounds, the
                // instrument reported the tank 9.36 m long because of two strapped-on canisters.
                let armour_rear = hull_mesh(vehicle).and_then(|mesh| {
                    let zs: Vec<f32> = mesh
                        .vertices()
                        .iter()
                        .filter(|v| v.material == vehicle_geometry::MaterialRole::RolledArmor)
                        .map(|v| v.position.z)
                        .collect();
                    (!zs.is_empty()).then(|| zs.into_iter().fold(f32::INFINITY, f32::min))
                });
                (armour_rear.map(|rear| gun.max.z.max(hull.max.z) - rear), MeasurementBasis::Mesh)
            }
            // Off the unit MESH, not the generator's radius field — the mesh disagreeing with
            // its own blueprint is exactly what this instrument must be able to catch.
            DimensionKind::RoadWheelDiameter => {
                (kin.as_ref().and_then(road_wheel_diameter_from_mesh), MeasurementBasis::Mesh)
            }
            // The ring race is interior — invisible to an exterior bake — so the anchor pins
            // the blueprint's declared ring against the dossier, and the report says so.
            DimensionKind::TurretRingDiameter => (
                blueprint.as_ref().map(|bp| 2.0 * bp.turret.ring_radius),
                MeasurementBasis::Blueprint,
            ),
            DimensionKind::FireLineHeight => {
                (Some(vehicle.mounts().gun_trunnion.translation.y), MeasurementBasis::Mounts)
            }
            DimensionKind::CupolaDiameter => {
                (cupola_diameter(vehicle, blueprint.as_ref()), MeasurementBasis::Mesh)
            }
            DimensionKind::TrackWidth => (
                kin.as_ref()
                    .and_then(|kin| track_link_unit_mesh(kin).bounds())
                    .map(|bounds| bounds.max.x - bounds.min.x),
                MeasurementBasis::Mesh,
            ),
            DimensionKind::TrackGauge => {
                (kin.as_ref().and_then(track_gauge_from_instances), MeasurementBasis::Instances)
            }
            DimensionKind::GroundClearance => {
                (ground_clearance(vehicle, blueprint.as_ref()), MeasurementBasis::Mesh)
            }
            DimensionKind::TrackLinkCountPerSide => (
                kin.as_ref().and_then(|kin| per_side_count(kin, GearPart::Link)),
                MeasurementBasis::Instances,
            ),
            DimensionKind::RoadWheelCount => (
                kin.as_ref().and_then(|kin| per_side_count(kin, GearPart::RoadWheel)),
                MeasurementBasis::Instances,
            ),
            DimensionKind::HeightToTurretRoofBare => {
                (bare_roof_height(vehicle, blueprint.as_ref()), MeasurementBasis::Mesh)
            }
            DimensionKind::FenderShelfHeight => {
                (fender_shelf_height(vehicle, blueprint.as_ref()), MeasurementBasis::Mesh)
            }
        };
        measurements.push(
            MeasuredDimension::new(target.clone(), measured_m.unwrap_or(f32::NAN))
                .with_basis(basis),
        );
    }
    Some(DimensionReport::new(vehicle.kind(), measurements))
}

/// The running gear to measure: a live blueprint's edited track when the Studio is running a
/// `--blueprint-file` override, otherwise the embedded blueprint's. Without this the fast loop
/// would measure and draw a belt the author is no longer editing.
fn gear_kinematics(
    vehicle: &BakedVehicle,
    live: Option<&VehicleBlueprint>,
) -> Option<RunningGearKinematics> {
    match live {
        Some(blueprint) => Some(RunningGearKinematics::from_track(&blueprint.track)),
        None => RunningGearKinematics::for_vehicle(vehicle.kind()),
    }
}

/// Diameter of the road-wheel unit mesh across its rolling plane. The wheel spins about X, so
/// the diameter is the larger of the Y/Z extents (they agree on a healthy wheel).
fn road_wheel_diameter_from_mesh(kin: &RunningGearKinematics) -> Option<f32> {
    let bounds = road_wheel_unit_mesh(kin).bounds()?;
    Some((bounds.max.y - bounds.min.y).max(bounds.max.z - bounds.min.z))
}

/// Track gauge as placed: twice the mean |x| of the link instances (both belts).
fn track_gauge_from_instances(kin: &RunningGearKinematics) -> Option<f32> {
    let mut sum = 0.0_f32;
    let mut count = 0_usize;
    for placement in running_gear_placements(kin, 0.0, 0.0) {
        if placement.part == GearPart::Link {
            sum += placement.transform.w_axis.x.abs();
            count += 1;
        }
    }
    (count > 0).then(|| 2.0 * sum / count as f32)
}

/// Instances of `part` that actually render on one side (x > 0), as a unit-less measurement.
fn per_side_count(kin: &RunningGearKinematics, part: GearPart) -> Option<f32> {
    let count = running_gear_placements(kin, 0.0, 0.0)
        .iter()
        .filter(|placement| placement.part == part && placement.transform.w_axis.x > 0.0)
        .count();
    (count > 0).then_some(count as f32)
}

/// Belly floor over the central strip of the hull submesh. The strip (55% of the hull's own
/// half-width) excludes fenders, sponsons, and the static gear brackets that hang beside the
/// tub — the tape goes under the belly, not under the running gear.
fn ground_clearance(vehicle: &BakedVehicle, blueprint: Option<&VehicleBlueprint>) -> Option<f32> {
    let mesh = &vehicle.submesh(SubmeshKind::Hull)?.mesh;
    // The strip this looks in is the HULL's own width, not a fraction of the widest thing on the
    // vehicle. It was `(bounds.max.x - bounds.min.x) * 0.5 * 0.55` — and the widest thing on a
    // T-54 is its FENDERS, so the strip came out at 0.90 while the belly plate's corners sit at
    // 1.03. The floor was never in the window at all, and what the instrument reported was
    // whatever chamfer happened to fall inside it: 0.444 against a documented 0.425.
    //
    // Clearance is the metal under the hull, across the hull. Without a blueprint (no hull width
    // to read) it degrades to the old fraction, which is the best a bare mesh allows.
    let strip = match blueprint {
        // A hair past the tub side, so the floor's own corners are inside the window.
        Some(bp) => bp.hull.lower_half_width.max(bp.hull.half_width) * 1.02,
        None => {
            let bounds = exterior_bounds(mesh)?;
            (bounds.max.x - bounds.min.x) * 0.5 * 0.55
        }
    };
    let mut min_y = f32::INFINITY;
    for vertex in mesh.vertices().iter().filter(|vertex| is_exterior(vertex.material)) {
        if vertex.position.x.abs() <= strip {
            min_y = min_y.min(vertex.position.y);
        }
    }
    min_y.is_finite().then_some(min_y)
}

/// The fender sheet's TOP surface, read in a thin strip at the shelf's OUTER edge over the
/// mid-hull run. The strip is outboard of the stowage line (boxes stop short of the edge) and
/// the z-window excludes the mudguard arches over the end wheels, so what remains in it is the
/// pressed sheet itself and its hem — whose highest point IS the sheet top. Measured off the
/// mesh, not the blueprint field, because the mesh disagreeing with its own blueprint is exactly
/// what this instrument must be able to catch. Vehicles without an authored fender (the recipe
/// fleet) return `None` and the row fails loudly if a pack ever anchors it.
fn fender_shelf_height(
    vehicle: &BakedVehicle,
    blueprint: Option<&VehicleBlueprint>,
) -> Option<f32> {
    let bp = blueprint?;
    let fender = bp.visual_detail().and_then(|hybrid| hybrid.fender.as_ref())?;
    let edge = fender.side_x + fender.half.x;
    let z_window = fender.half.z * 0.75;
    let mesh = &vehicle.submesh(SubmeshKind::Hull)?.mesh;
    let mut max_y = f32::NEG_INFINITY;
    for vertex in mesh.vertices().iter().filter(|vertex| is_exterior(vertex.material)) {
        if (vertex.position.x.abs() - edge).abs() <= 0.02 && vertex.position.z.abs() <= z_window {
            max_y = max_y.max(vertex.position.y);
        }
    }
    max_y.is_finite().then_some(max_y)
}

/// Armor skin only — turret roof/cupola measurements must not read the AA machine gun or other
/// steel furniture as the casting.
fn is_armor_skin(material: MaterialRole) -> bool {
    matches!(material, MaterialRole::CastArmor | MaterialRole::RolledArmor)
}

/// Structural roof height: the turret casting's highest armor point OUTSIDE the blueprint's
/// cupola disc. Flush hatch lids read as roof plane by design. Without a blueprint (no cupola
/// knowledge) this degrades to the silhouette apex.
fn bare_roof_height(vehicle: &BakedVehicle, blueprint: Option<&VehicleBlueprint>) -> Option<f32> {
    let mesh = &vehicle.submesh(SubmeshKind::Turret)?.mesh;
    let Some(bp) = blueprint else {
        return submesh_bounds(vehicle, SubmeshKind::Turret).map(|bounds| bounds.max.y);
    };
    let (cx, cz) = (bp.turret.cupola_x, bp.turret.cupola_z);
    let exclusion = bp.turret.cupola_radius + 0.06;
    let mut max_y = f32::NEG_INFINITY;
    for vertex in mesh.vertices().iter().filter(|vertex| is_armor_skin(vertex.material)) {
        let (dx, dz) = (vertex.position.x - cx, vertex.position.z - cz);
        if (dx * dx + dz * dz).sqrt() > exclusion {
            max_y = max_y.max(vertex.position.y);
        }
    }
    max_y.is_finite().then_some(max_y)
}

/// Commander-cupola external diameter: twice the largest radial reach of armor-skin vertices
/// inside the blueprint's cupola disc, above the bare roof — a horizontal tape around the drum.
fn cupola_diameter(vehicle: &BakedVehicle, blueprint: Option<&VehicleBlueprint>) -> Option<f32> {
    let bp = blueprint?;
    if bp.turret.cupola_radius <= 0.0 {
        return None;
    }
    let roof = bare_roof_height(vehicle, blueprint)?;
    let mesh = &vehicle.submesh(SubmeshKind::Turret)?.mesh;
    let (cx, cz) = (bp.turret.cupola_x, bp.turret.cupola_z);
    let capture = bp.turret.cupola_radius + 0.06;
    let mut max_radial = 0.0_f32;
    for vertex in mesh.vertices().iter().filter(|vertex| is_armor_skin(vertex.material)) {
        // ABOVE the roof, which is what the doc comment says and what a tape measure round a
        // cupola actually touches. The old cut sat 50 mm BELOW the roof, so on a wide drum it
        // caught the roof plate itself and reported the plate's reach as the cupola's diameter.
        if vertex.position.y <= roof {
            continue;
        }
        let (dx, dz) = (vertex.position.x - cx, vertex.position.z - cz);
        let radial = (dx * dx + dz * dz).sqrt();
        if radial <= capture {
            max_radial = max_radial.max(radial);
        }
    }
    (max_radial > 0.0).then_some(2.0 * max_radial)
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

pub(crate) fn is_exterior(material: MaterialRole) -> bool {
    !matches!(
        material,
        MaterialRole::InteriorPrimer | MaterialRole::InteriorMachinery | MaterialRole::Ammunition
    )
}

fn visual_hull_bounds(
    vehicle: &BakedVehicle,
    kin: Option<&RunningGearKinematics>,
) -> Option<MeshBounds> {
    let mut bounds = submesh_bounds(vehicle, SubmeshKind::Hull)?;
    if let Some(kin) = kin {
        bounds = bounds.union(running_gear_bounds(kin)?);
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
    let swing_arm_left = vehicle_geometry::swing_arm_unit_mesh_left(kin);
    let damper = vehicle_geometry::damper_unit_mesh(kin);
    let damper_left = vehicle_geometry::damper_unit_mesh_left(kin);
    let return_roller = vehicle_geometry::return_roller_unit_mesh(kin);
    let mut bounds: Option<MeshBounds> = None;
    for placement in running_gear_placements(kin, 0.0, 0.0) {
        let mesh = match placement.part {
            GearPart::RoadWheel => &road_wheel,
            GearPart::Idler => &idler,
            GearPart::Sprocket => &sprocket,
            GearPart::Link => &link,
            GearPart::SwingArm => &swing_arm,
            GearPart::SwingArmLeft => &swing_arm_left,
            GearPart::Damper => &damper,
            GearPart::DamperLeft => &damper_left,
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
