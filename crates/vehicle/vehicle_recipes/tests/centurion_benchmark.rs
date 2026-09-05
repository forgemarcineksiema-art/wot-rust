//! The Centurion Mk 3 shape cage: locks the anatomy the British line was born with — the
//! fleet's first authored SKIRTS (visible sheet == armor screen plane), the Horstmann bogie
//! pairs under them, the steepest glacis in the game standing on its armor plane, the cast
//! dome with the bustle bin, and the clean unbraked 20-pounder. Each lock names a defect that
//! would silently un-Centurion the tank.

use game_core::{ArmorZone, VehicleBlueprint, VehicleKind, vehicle_armor_volumes};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::bake_vehicle;

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

    // And it genuinely hides the upper half of the wheels: skirt bottom at or below the axle,
    // skirt top above the wheel tops.
    assert!(skirt.bottom_y <= bp.track.axle_y() + 1.0e-6);
    assert!(skirt.top_y > bp.track.axle_y() + bp.track.wheel_radius);
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
