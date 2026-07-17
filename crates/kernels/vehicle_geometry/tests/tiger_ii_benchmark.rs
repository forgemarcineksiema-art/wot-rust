//! The Tiger II shape cage: locks the sloped-school anatomy the blueprint migration bought —
//! the fleet's longest glacis and leaned upper sides standing ON the armor planes, the faceted
//! Henschel prism with its bustle closing the rear armor plane, the overlapped nine-wheel
//! rows, and the five-metre braked KwK 43. Each lock names a defect that would silently
//! un-King-Tiger the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::TigerII).expect("Tiger II has a blueprint")
}

/// The 150 mm @50° glacis is the fleet's longest sloped plate, and the visible plate lies ON
/// the armor volume's plane — what you see leaning 50° is what the penetration model resolves.
#[test]
fn the_visible_glacis_is_the_armor_plane_and_the_longest_in_the_fleet() {
    let bp = blueprint();
    assert!((bp.armor.hull_front.0 - 50.0).abs() < 1.0e-6);
    let baked = bake_vehicle(VehicleKind::TigerII).expect("Tiger II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::TigerII).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;

    let glacis = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::UpperGlacis)
        .expect("glacis plane");
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| {
            point.y > bp.hull.sponson_y - cy - 1.0e-3
                && (glacis.normal.dot(*point) - glacis.offset).abs() < 1.0e-3
        })
        .count();
    assert!(on_plane >= 4, "the visible glacis must lie on the armor plane: {on_plane}");

    // The slope run in plan: over a metre of z between the fold and the deck edge — longer
    // than any other plate in the lineup (the Tiger I's whole run is ~15 cm).
    let run = (bp.hull.deck_y - bp.hull.sponson_y) * bp.hull.glacis_slope_deg.to_radians().tan();
    assert!(run > 1.0, "the King Tiger glacis is the fleet's longest: {run}");
}

/// The upper hull sides LEAN their 25°: the armor facet says so, the armor plane carries it,
/// and the visible wall narrows from the sponson beam to the deck along that exact plane.
#[test]
fn the_leaned_upper_sides_lie_on_the_armor_side_planes() {
    let bp = blueprint();
    assert!((bp.armor.hull_side.0 - 25.0).abs() < 1.0e-6);
    let baked = bake_vehicle(VehicleKind::TigerII).expect("Tiger II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::TigerII).expect("armor volumes");
    let cy = bp.hull.hitbox_center_y;

    let side = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x > 0.5)
        .expect("right side plane");
    assert!((side.normal.y.asin().to_degrees() - 25.0).abs() < 1.0e-3, "the plane leans 25°");
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| point.x > 0.5 && (side.normal.dot(*point) - side.offset).abs() < 1.0e-3)
        .count();
    assert!(on_plane >= 4, "the visible side wall must lie on the armor plane: {on_plane}");

    // And the wall genuinely narrows: the deck is visibly slimmer than the sponson beam.
    let deck_width = hull_mesh
        .vertices()
        .iter()
        .filter(|v| (v.position.y - bp.hull.deck_y).abs() < 1.0e-3)
        .map(|v| v.position.x)
        .fold(0.0_f32, f32::max);
    assert!(
        deck_width < bp.hull.half_width - 0.30,
        "the deck edge sits well inboard of the sponson beam: {deck_width}"
    );
}

/// The Henschel is a plate prism whose walls carry their real slopes — 10° front (with the
/// mantlet patch), 21° converging sides, 20° bustle rear — and the visible bustle wall closes
/// the rear armor plane at the ring seat.
#[test]
fn the_henschel_prism_carries_its_plate_slopes() {
    let bp = blueprint();
    let volumes = vehicle_armor_volumes(VehicleKind::TigerII).expect("armor volumes");
    assert_eq!(volumes.turret.planes.len(), 6, "welded box: a prism, not a swept dome");
    let side = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretSide && plane.normal.x > 0.5)
        .expect("right wall");
    assert!((side.normal.y.asin().to_degrees() - 21.0).abs() < 1.0e-3);
    let rear = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretRear)
        .expect("rear wall");
    assert!((rear.normal.y.asin().to_degrees() - 20.0).abs() < 1.0e-3);

    let baked = bake_vehicle(VehicleKind::TigerII).expect("Tiger II bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    // At the ring seat the widest wall stands on the armor plane and the bustle's rear wall
    // closes the rear plane; at the roof the walls have converged by their slopes.
    let at_ring: Vec<Vec3> = turret_mesh
        .vertices()
        .iter()
        .map(|v| v.position)
        .filter(|p| (p.y - bp.turret.ring_y).abs() < 1.0e-3)
        .collect();
    let widest = at_ring.iter().map(|p| p.x).fold(0.0_f32, f32::max);
    let rearmost = at_ring.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
    assert!((widest - bp.turret.plan_half_width).abs() < 0.02, "wall on the plane: {widest}");
    assert!(
        (rearmost - (bp.turret.ring_z - bp.turret.plan_half_length)).abs() < 0.02,
        "the bustle closes the rear armor plane: {rearmost}"
    );
    let roof_widest = turret_mesh
        .vertices()
        .iter()
        .filter(|v| (v.position.y - bp.turret.roof_y).abs() < 1.0e-3)
        .map(|v| v.position.x)
        .fold(0.0_f32, f32::max);
    let expected = bp.turret.plan_half_width
        - (bp.turret.roof_y - bp.turret.ring_y) * bp.turret.side_slope_deg.to_radians().tan();
    assert!(
        (roof_widest - expected).abs() < 0.02,
        "the roof edge sits where the 21° wall puts it: {roof_widest} vs {expected}"
    );
}

/// Nine overlapped wheels per side in two rows (the Tiger II stagger, shallower than the
/// Tiger I interleave), no return rollers, and the discs never interpenetrate.
#[test]
fn nine_overlapped_wheels_in_two_rows_without_rollers() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 9, "nine axles per side");
    assert_eq!(bp.track.return_rollers, 0);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::TigerII).expect("blueprint gear");
    let wheels: Vec<f32> = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
        .map(|p| p.transform.w_axis.x)
        .collect();
    // The Schachtellaufwerk sandwich (W1 PR-T1.2): every outer-row axle carries a second
    // deep-plane disc, so 9 axles render 14 discs per side in three planes.
    assert_eq!(wheels.len(), 14);
    let mut rows = wheels.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
    assert_eq!(rows.len(), 3, "three interleaved wheel planes, got {rows:?}");
    assert!(bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width, "discs must not merge");
}

/// The KwK 43 L/71: five metres of barrel past the trunnion tipped with its brake, carrying
/// the 10.29 m overall length (gun forward) the references document, and no bore evacuator.
#[test]
fn the_kwk43_carries_five_metres_of_braked_barrel() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_some(), "the KwK 43 wears its brake");
    assert!(bp.gun.evacuator.is_none(), "no bore evacuator on a KwK 43");
    let reach = bp.gun.muzzle_z - bp.gun.trunnion_z;
    assert!(reach >= 5.0 - 1.0e-6, "five metres of barrel: {reach}");
    // Overall length gun forward: rear hull to muzzle = half_len + muzzle_z = 10.29 m.
    assert!(((bp.hull.half_len + bp.gun.muzzle_z) - 10.29).abs() < 0.01);
}

/// The migrated body is the RESEARCHED King Tiger: 7.38 m hull in a 7.48 m box, 3.755 m over
/// the tracks, 3.09 m tall — not the old 8.0 m box with vertical sides.
#[test]
fn the_hitbox_is_the_researched_body_not_the_legacy_stretch() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::TigerII);
    assert!((hitbox.half_length_m - 3.74).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.89).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 3.09).abs() < 1.0e-6, "3.09 m tall");
    assert!((bp.hull.half_len - 3.69).abs() < 1.0e-6, "the documented 7.38 m hull");
    assert!((bp.track.outer_x - 1.87).abs() < 1.0e-6, "3.75 m over the combat tracks");
}
