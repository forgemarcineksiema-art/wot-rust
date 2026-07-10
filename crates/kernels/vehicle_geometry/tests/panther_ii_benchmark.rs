//! The Panther II shape cage: locks the wedge anatomy the blueprint migration bought — the
//! steepest German glacis standing ON the armor plane, the 29° leaned sides, the deliberately
//! narrow Schmalturm converging to a slim roof, and the seven overlapped steel wheels. Each
//! lock names a defect that would silently un-Panther the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::PantherII).expect("Panther II has a blueprint")
}

/// The 55° ramp is the steepest GERMAN glacis in the fleet, and the visible plate lies ON the
/// armor volume's plane.
#[test]
fn the_ramp_is_the_steepest_german_glacis_on_the_armor_plane() {
    let bp = blueprint();
    assert!((bp.armor.hull_front.0 - 55.0).abs() < 1.0e-6);
    for kind in [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::Jagdtiger] {
        let other = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        assert!(other.armor.hull_front.0 < 55.0, "{kind:?} must not out-slope the Panther II");
    }

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
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
    assert!(on_plane >= 4, "the visible ramp must lie on the armor plane: {on_plane}");
}

/// The upper sides lean their 29° on the armor plane — the Panther family's sponson rake.
#[test]
fn the_upper_sides_lean_their_29_degrees() {
    let bp = blueprint();
    assert!((bp.armor.hull_side.0 - 29.0).abs() < 1.0e-6);
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
    let side = volumes.hull[0]
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::HullSide && plane.normal.x > 0.5)
        .expect("right side plane");
    assert!((side.normal.y.asin().to_degrees() - 29.0).abs() < 1.0e-3);

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let cy = bp.hull.hitbox_center_y;
    let on_plane = hull_mesh
        .vertices()
        .iter()
        .map(|vertex| vertex.position - Vec3::Y * cy)
        .filter(|point| point.x > 0.5 && (side.normal.dot(*point) - side.offset).abs() < 1.0e-3)
        .count();
    assert!(on_plane >= 4, "the visible side wall must lie on the armor plane: {on_plane}");
}

/// The Schmalturm is genuinely NARROW: its beam is the smallest turret beam in the German
/// line, its front plate is barely wider than the Saukopf, and its walls stand on the armor
/// prism planes, converging hard to a slim roof.
#[test]
fn the_schmalturm_is_the_narrow_turret_of_the_german_line() {
    let bp = blueprint();
    for kind in [VehicleKind::TigerI, VehicleKind::TigerII] {
        let other = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        assert!(
            bp.turret.plan_half_width < other.turret.plan_half_width - 0.1,
            "{kind:?} must be broader than the Schmalturm"
        );
    }
    let volumes = vehicle_armor_volumes(VehicleKind::PantherII).expect("armor volumes");
    assert_eq!(volumes.turret.planes.len(), 6, "welded box: a prism");
    let side = volumes
        .turret
        .planes
        .iter()
        .find(|plane| plane.zone == ArmorZone::TurretSide && plane.normal.x > 0.5)
        .expect("right cheek");
    assert!((side.normal.y.asin().to_degrees() - 25.0).abs() < 1.0e-3, "hard-converging cheeks");

    let baked = bake_vehicle(VehicleKind::PantherII).expect("Panther II bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let widest_at_ring = turret_mesh
        .vertices()
        .iter()
        .filter(|v| (v.position.y - bp.turret.ring_y).abs() < 1.0e-3)
        .map(|v| v.position.x)
        .fold(0.0_f32, f32::max);
    assert!(
        (widest_at_ring - bp.turret.plan_half_width).abs() < 0.02,
        "the cheek stands on the armor plane: {widest_at_ring}"
    );
}

/// Seven overlapped steel wheels per side in two rows — the Tiger II school, not the Panther's
/// rubber dish — with no return rollers.
#[test]
fn seven_overlapped_steel_wheels_without_rollers() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 7, "seven axles per side");
    assert_eq!(bp.track.return_rollers, 0);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::PantherII).expect("blueprint gear");
    let wheels: Vec<f32> = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::RoadWheel && p.transform.w_axis.x > 0.0)
        .map(|p| p.transform.w_axis.x)
        .collect();
    assert_eq!(wheels.len(), 7);
    let mut rows = wheels.clone();
    rows.sort_by(f32::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1.0e-4);
    assert_eq!(rows.len(), 2, "two wheel rows, got {rows:?}");
    assert!(bp.track.overlap_inner_dx >= 2.0 * kin.wheel_half_width, "discs must not merge");
}

/// The KwK 42 L/70 wears a small brake and no evacuator, reaching the documented ~8.9 m
/// overall — a MEDIUM gun, visibly slighter than the Tiger guns beside it.
#[test]
fn the_kwk42_is_a_slighter_braked_gun() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_some());
    assert!(bp.gun.evacuator.is_none());
    assert!(bp.gun.barrel_radius < 0.09, "a 7.5 cm barrel, not an 8.8");
    assert!(((bp.hull.half_len + bp.gun.muzzle_z) - 9.03).abs() < 0.1, "~9 m overall");
}

/// The migrated body is the RESEARCHED Panther II: 6.87 m hull in a 6.98 m box, 3.42 m over
/// the tracks, 2.99 m tall — not the old 7.4 m box with vertical sides.
#[test]
fn the_hitbox_is_the_researched_body_not_the_legacy_stretch() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::PantherII);
    assert!((hitbox.half_length_m - 3.49).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.73).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 2.99).abs() < 1.0e-6);
    assert!((bp.hull.half_len - 3.435).abs() < 1.0e-6, "the documented 6.87 m hull");
    assert!((bp.track.outer_x - 1.70).abs() < 1.0e-6, "3.4 m over the tracks");
}
