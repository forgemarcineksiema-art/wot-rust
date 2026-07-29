//! WHAT YOU SEE IS WHAT YOU SHOOT — for the cast turret.
//!
//! The IS-3's pike bow has this lock already (`is3_pike.rs`): every visible bow vertex must lie
//! inside the armour half-spaces it is shot against, and each armour plane must actually be
//! touched by metal. The T-54's turret — the vehicle's entire armour argument — has never had it.
//!
//! It needed it. The dome's armour volume was swept at ONE radius in every direction while the
//! casting is an egg: wider than long across the cheeks is not what a T-54 is, and the mesh runs
//! further forward than the circle it was shot against. So the front of the casting stood outside
//! its own armour (shells passing through steel that was not there to stop them) while the flanks
//! had armour standing outside the metal (shells stopped by air).

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::GeometryMesh;

/// The CASTING itself — not the whole turret submesh. Handrails, periscopes and the cupola are
/// fittings and separate parts: a shell passes through a handrail, so holding one to the
/// casting's armour volume would be measuring the wrong thing.
fn casting_mesh() -> GeometryMesh {
    vehicle_build::t54_description()
        .parts
        .iter()
        .find(|part| part.key.name == "turret_shell")
        .expect("the T-54 turret is one lofted casting")
        .mesh()
}

/// Signed distance of `point` outside `plane` — positive means the point is OUT of the volume.
fn outside_by(plane: &game_core::TaggedPlane, point: Vec3) -> f32 {
    plane.normal.dot(point) - plane.offset
}

#[test]
fn every_visible_turret_vertex_is_inside_the_armour_it_is_shot_against() {
    let turret = casting_mesh();
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("armor volumes");
    let cy = blueprint.hull.hitbox_center_y;

    // The roof plane and the ring underside are height limits, not plan limits: the cupola and
    // the ring collar deliberately cross them (they are separate parts with their own armour).
    // What this test is about is the CASTING's plan — the sector planes.
    let sectors: Vec<_> =
        volumes.turret.planes.iter().filter(|plane| plane.normal.y.abs() < 0.999).collect();
    assert!(sectors.len() >= 12, "a cast dome is swept into sectors, found {}", sectors.len());

    let mut worst = 0.0_f32;
    let mut worst_point = Vec3::ZERO;
    for vertex in turret.vertices() {
        let point = vertex.position - Vec3::Y * cy;
        // A point is outside the convex volume only if it is outside EVERY plane's… no: outside
        // the volume means outside AT LEAST one plane. But a swept dome's sectors only bound
        // their own azimuth, so judge each vertex against the sector facing it.
        let azimuth = point.x.atan2(point.z - volumes.turret_ring_z);
        let facing = Vec3::new(azimuth.sin(), 0.0, azimuth.cos());
        let Some(plane) = sectors
            .iter()
            .max_by(|a, b| {
                let da = Vec3::new(a.normal.x, 0.0, a.normal.z).normalize_or_zero().dot(facing);
                let db = Vec3::new(b.normal.x, 0.0, b.normal.z).normalize_or_zero().dot(facing);
                da.total_cmp(&db)
            })
            .copied()
        else {
            continue;
        };
        let out = outside_by(plane, point);
        if out > worst {
            worst = out;
            worst_point = point;
        }
    }
    println!("TURRET ARMOUR GAP: worst {worst:.4} m outside at {worst_point:?}");
    assert!(
        worst <= 0.01,
        "visible turret metal stands {worst:.3} m outside its armour volume at {worst_point:?} — \
         a shell passes through steel that is not there to stop it"
    );
}

/// The other direction: armour that reaches past the metal stops shells with air.
#[test]
fn the_armour_dome_does_not_stand_proud_of_the_casting() {
    let turret = casting_mesh();
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let volumes = vehicle_armor_volumes(VehicleKind::T54_1951).expect("armor volumes");
    let cy = blueprint.hull.hitbox_center_y;

    let mut worst_slack = 0.0_f32;
    let mut worst_zone = ArmorZone::TurretFront;
    for plane in &volumes.turret.planes {
        if plane.normal.y.abs() > 0.999 {
            continue;
        }
        // How close does the metal come to this plane?
        let mut closest = f32::INFINITY;
        for vertex in turret.vertices() {
            let point = vertex.position - Vec3::Y * cy;
            // Only metal on this plane's side of the casting can touch it.
            let radial = Vec3::new(point.x, 0.0, point.z - volumes.turret_ring_z);
            let facing = Vec3::new(plane.normal.x, 0.0, plane.normal.z).normalize_or_zero();
            if radial.normalize_or_zero().dot(facing) < 0.86 {
                continue;
            }
            closest = closest.min(-outside_by(plane, point));
        }
        if closest.is_finite() && closest > worst_slack {
            worst_slack = closest;
            worst_zone = plane.zone;
        }
    }
    println!("TURRET ARMOUR SLACK: worst {worst_slack:.4} m of air on {worst_zone:?}");
    assert!(
        worst_slack <= 0.05,
        "the armour dome reaches {worst_slack:.3} m past the metal on {worst_zone:?} — shells \
         stop in air that looks like sky"
    );
}
