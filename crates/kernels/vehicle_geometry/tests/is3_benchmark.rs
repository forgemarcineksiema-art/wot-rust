//! The IS-3 shape cage, completing the per-vehicle benchmark set: locks the heavy's anatomy
//! beyond the pike (which `is3_pike.rs` already guards) — the flattened overhanging turtle
//! dome, the braked evacuator-less D-25T, the rollered top run over six small wheels, the
//! rear fuel drums, and the LOW silhouette that is the whole point of the design. Each lock
//! names a defect that would silently un-IS-3 the tank.

use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_geometry::{
    GearPart, RunningGearKinematics, SubmeshKind, bake_vehicle, running_gear_placements,
};

fn blueprint() -> VehicleBlueprint {
    VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("IS-3 has a blueprint")
}

/// The pike bow is TWO plates swept ±38° in plan: angling the hull genuinely flattens one
/// face toward the shooter — the anti-angling trade the design made.
#[test]
fn the_pike_carries_its_38_degree_plan_sweep() {
    let bp = blueprint();
    assert!((bp.hull.pike_sweep_deg - 38.0).abs() < 1.0e-6);
    let volumes = game_core::vehicle_armor_volumes(VehicleKind::IS3).expect("armor volumes");
    let headings: Vec<f32> = volumes.hull[0]
        .planes
        .iter()
        .filter(|plane| plane.zone == game_core::ArmorZone::UpperGlacis)
        .map(|plane| plane.normal.x.atan2(plane.normal.z).to_degrees())
        .collect();
    assert_eq!(headings.len(), 2, "two swept bow planes");
    assert!(
        (headings[0] + headings[1]).abs() < 1.0e-3,
        "the sweep is symmetric about the ridge: {headings:?}"
    );
    assert!(headings.iter().any(|h| (h.abs() - 38.0).abs() < 2.0), "±38° in plan: {headings:?}");
}

/// The turtle dome: a genuinely flattened casting that OVERHANGS its ring — steeper at the
/// face than any other dome in the fleet, and wider than the race it sits on.
#[test]
fn the_turtle_dome_overhangs_its_ring_and_out_slopes_every_dome() {
    let bp = blueprint();
    assert!(
        bp.turret.base_radius > bp.turret.ring_radius + 0.2,
        "the casting overhangs the ring: {} vs {}",
        bp.turret.base_radius,
        bp.turret.ring_radius
    );
    for kind in [VehicleKind::T54_1951, VehicleKind::T55A] {
        let other = VehicleBlueprint::for_vehicle(kind).expect("blueprint");
        assert!(
            bp.turret.front_slope_deg > other.turret.front_slope_deg + 10.0,
            "{kind:?} must not out-slope the IS-3 dome face"
        );
    }
}

/// The D-25T wears its double-baffle brake and NO bore evacuator — and it is the fattest
/// turreted barrel in the lineup.
#[test]
fn the_d25t_is_a_braked_evacuatorless_122() {
    let bp = blueprint();
    assert!(bp.gun.muzzle_brake.is_some(), "the 122 wears its brake");
    assert!(bp.gun.evacuator.is_none(), "no evacuator on a D-25T");
    assert!(bp.gun.barrel_radius > 0.10, "a 122 mm barrel");
}

/// The IS family carries its top run on three small return rollers per side — the heavy's
/// look against the wheel-riding Soviet mediums — over six small 550 mm wheels.
#[test]
fn three_return_rollers_carry_the_top_run_over_six_small_wheels() {
    let bp = blueprint();
    assert_eq!(bp.track.wheel_count, 6);
    assert!((bp.track.wheel_radius - 0.275).abs() < 1.0e-6, "550 mm wheels");
    assert_eq!(bp.track.return_rollers, 3);
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::IS3).expect("blueprint gear");
    let rollers = running_gear_placements(&kin, 0.0, 0.0)
        .iter()
        .filter(|p| p.part == GearPart::ReturnRoller)
        .count();
    assert_eq!(rollers, 6, "three rollers per side");
}

/// The rear fuel drums: one cylindrical tank lying along each rear fender — visible stowage
/// metal standing proud of the shelf on both sides, aft of the hull midpoint.
#[test]
fn the_rear_fenders_carry_the_external_fuel_drums() {
    let bp = blueprint();
    let baked = bake_vehicle(VehicleKind::IS3).expect("IS-3 bakes");
    let hull_mesh = &baked.submesh(SubmeshKind::Hull).expect("hull submesh").mesh;
    let shelf_y = bp.hull.sponson_y + 0.05;
    for side in [-1.0_f32, 1.0] {
        let drum_apex = hull_mesh
            .vertices()
            .iter()
            .map(|v| v.position)
            .filter(|p| p.x * side > bp.hull.lower_half_width && p.z < -1.5)
            .map(|p| p.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            drum_apex > shelf_y + 0.22,
            "side {side}: a drum must stand proud of the rear fender: apex {drum_apex}"
        );
    }
}

/// The LOW silhouette is the design's whole point: the IS-3 heavy stands over half a metre
/// SHORTER than every German heavy while carrying the fleet's biggest turreted gun.
#[test]
fn the_heavy_is_lower_than_every_german() {
    let is3 = game_core::HitboxProfile::for_vehicle(VehicleKind::IS3);
    let is3_top = is3.center_y_m + is3.half_height_m;
    assert!((is3_top - 2.49).abs() < 1.0e-6, "2.44 m tank in a 2.49 m box");
    for kind in [VehicleKind::TigerI, VehicleKind::TigerII, VehicleKind::PantherII] {
        let other = game_core::HitboxProfile::for_vehicle(kind);
        let top = other.center_y_m + other.half_height_m;
        assert!(is3_top < top - 0.4, "{kind:?} must tower over the IS-3: {top} vs {is3_top}");
    }
    // And the visible dome apex agrees with the box.
    let baked = bake_vehicle(VehicleKind::IS3).expect("IS-3 bakes");
    let apex = baked
        .submesh(SubmeshKind::Turret)
        .expect("turret submesh")
        .mesh
        .vertices()
        .iter()
        .map(|v| v.position.y)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(apex <= is3_top + 0.05, "the dome stays inside the low box: {apex}");
}
