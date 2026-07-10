//! The Centurion Mk 3 shape cage: locks the anatomy the British line was born with — the
//! fleet's first authored SKIRTS (visible sheet == armor screen plane), the Horstmann bogie
//! pairs under them, the steepest glacis in the game standing on its armor plane, the cast
//! dome with the bustle bin, and the clean unbraked 20-pounder. Each lock names a defect that
//! would silently un-Centurion the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use glam::Vec3;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::Centurion).expect("Centurion has a blueprint")
}

/// The first authored skirt in the fleet: the armor volumes bake a thin spaced-screen pair
/// outside the tracks, and the visible bazooka plate stands EXACTLY on that plane, hiding the
/// upper half of the road wheels.
#[test]
fn the_bazooka_plates_are_the_armor_screen_and_hide_the_wheel_tops() {
    let bp = blueprint();
    let skirt = bp.hull.skirt.expect("the Centurion authors a skirt");
    let volumes = vehicle_armor_volumes(VehicleKind::Centurion).expect("armor volumes");
    assert_eq!(volumes.hull.len(), 6, "hull + tub + two tracks + two skirt screens");
    for screen in &volumes.hull[4..] {
        assert!(screen.planes.iter().all(|plane| plane.zone == ArmorZone::Skirt));
    }

    // The visible sheet stands on the armor plane.
    let baked = bake_vehicle(VehicleKind::Centurion).expect("Centurion bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let outer_x = bp.track.outer_x + skirt.standoff_m + skirt.thickness_m;
    let on_plane =
        hull_mesh.vertices().iter().filter(|v| (v.position.x - outer_x).abs() < 1.0e-3).count();
    assert!(on_plane >= 4, "the visible plate stands on the screen plane: {on_plane}");

    // And it genuinely hides the upper half of the wheels: skirt bottom at or below the axle.
    assert!(skirt.bottom_y <= bp.track.bottom_y + bp.track.wheel_radius + 1.0e-6);
    assert!(skirt.top_y > bp.track.bottom_y + 2.0 * bp.track.wheel_radius - 0.05);
}

/// Horstmann: six wheels in three bogie PAIRS — tight inside a bogie, a real gap between
/// bogies — with three return rollers carrying the top run.
#[test]
fn three_horstmann_bogie_pairs_with_return_rollers() {
    let bp = blueprint();
    let stations = bp.track.wheel_stations();
    assert_eq!(stations.len(), 6);
    let pair_gap = stations[1] - stations[0];
    let bogie_gap = stations[2] - stations[1];
    assert!(
        bogie_gap > pair_gap * 1.5,
        "the gap between bogies must dwarf the in-pair pitch: {bogie_gap} vs {pair_gap}"
    );
    assert_eq!(bp.track.return_rollers, 3);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::Centurion).expect("blueprint gear");
    let placements = running_gear_placements(&kin, 0.0, 0.0);
    assert_eq!(placements.iter().filter(|p| p.part == GearPart::RoadWheel).count(), 12);
    assert_eq!(placements.iter().filter(|p| p.part == GearPart::ReturnRoller).count(), 6);
}

/// The 76 mm glacis leans 57° — out-sloping every German plate (only the Soviet 60° school
/// leans harder) — and the visible plate lies ON the armor volume's plane.
#[test]
fn the_glacis_out_slopes_every_german_plate_on_its_armor_plane() {
    let bp = blueprint();
    assert!((bp.armor.hull_front.0 - 57.0).abs() < 1.0e-6);
    for kind in
        [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::Jagdtiger, VehicleKind::PantherII]
    {
        let other = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        assert!(other.armor.hull_front.0 < 57.0, "{kind:?} must not out-slope the Centurion");
    }
    let baked = bake_vehicle(VehicleKind::Centurion).expect("Centurion bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let volumes = vehicle_armor_volumes(VehicleKind::Centurion).expect("armor volumes");
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
}

/// The cast Mk 3 dome carries the bustle stowage bin — turret metal reaches the rear of the
/// turret plan, well behind the casting's own bustle.
#[test]
fn the_bustle_bin_closes_the_turret_plan() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::Centurion).expect("Centurion bakes");
    let turret_mesh = &baked.submesh(SubmeshKind::Turret).expect("turret submesh").mesh;
    let rearmost =
        turret_mesh.vertices().iter().map(|v| v.position.z).fold(f32::INFINITY, f32::min);
    let plan_rear = bp.turret.ring_z - bp.turret.plan_half_length;
    assert!(
        (rearmost - plan_rear).abs() < 0.05,
        "the bin reaches the rear of the plan: {rearmost} vs {plan_rear}"
    );
}

/// The 20-pounder Type A is a CLEAN tube: no muzzle brake, no fume extractor — the barrel
/// stays a plain cylinder to the muzzle, unlike every German gun beside it.
#[test]
fn the_20pdr_type_a_is_a_clean_tube() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_none(), "Type A wears no brake");
    assert!(bp.gun.evacuator.is_none(), "Type A wears no fume extractor");
    let baked = bake_vehicle(VehicleKind::Centurion).expect("Centurion bakes");
    let gun_mesh = &baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh;
    let swell = gun_mesh
        .vertices()
        .iter()
        .filter(|v| v.position.z > bp.gun.trunnion_z + 1.0)
        .map(|v| Vec3::new(v.position.x, v.position.y - bp.gun.trunnion_y, 0.0).length())
        .fold(0.0_f32, f32::max);
    assert!(
        swell < bp.gun.barrel_radius * 1.2,
        "no swelling anywhere along the free barrel: {swell}"
    );
}

/// The born body is the researched Centurion: 7.60 m hull in a 7.72 m box, ~3.34 m over the
/// skirts, 2.99 m tall.
#[test]
fn the_hitbox_is_the_researched_body() {
    let bp = blueprint();
    let hitbox = game_core::HitboxProfile::for_vehicle(VehicleKind::Centurion);
    assert!((hitbox.half_length_m - 3.86).abs() < 1.0e-6);
    assert!((hitbox.half_width_m - 1.70).abs() < 1.0e-6);
    assert!(((hitbox.center_y_m + hitbox.half_height_m) - 2.99).abs() < 1.0e-6);
    assert!((bp.hull.half_len - 3.80).abs() < 1.0e-6, "the documented 7.60 m hull");
}
