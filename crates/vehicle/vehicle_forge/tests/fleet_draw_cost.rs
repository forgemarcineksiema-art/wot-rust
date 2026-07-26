//! THE MEASURED COST OF PUTTING THE FLEET ON SCREEN.
//!
//! `VEHICLE_BUDGETS` bounds the STATIC bake and says so plainly — "runtime running gear is
//! excluded: wheels, suspension, end wheels, and links are animated instances". That is correct
//! for what it guards, and it means the number everyone quotes for a vehicle is not the number
//! the GPU draws. Road wheels, swing arms, idler, sprocket and every shoe link are real
//! triangles, instanced per side, and nothing wrote their total down.
//!
//! This file writes it down. It asserts almost nothing on purpose: it PRINTS the per-vehicle and
//! whole-battle table so a density decision (how dense may the fleet get before vehicle LOD stops
//! being optional?) is taken against a measurement instead of an estimate.
//!
//! It lives in `vehicle_forge`, not in `vehicle_geometry`, so it can resolve each vehicle through
//! [`authoritative_baked_vehicle`] — the mesh the game actually draws. Measured from
//! `bake_vehicle` instead, the T-54 row would report its unused procedural recipe (1.6k triangles)
//! rather than the hybrid the client ships (15.1k), and a cost table that quotes a mesh nobody
//! renders is worse than no table.

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{
    GearPart, RunningGearKinematics, idler_unit_mesh, road_wheel_unit_mesh,
    running_gear_placements, sprocket_unit_mesh, swing_arm_unit_mesh, track_link_unit_mesh,
};

/// A 7v7 battle: the fleet size the game is designed around.
const BATTLE_TANKS: usize = 14;

struct Cost {
    static_tris: usize,
    gear_tris: usize,
    gear_instances: usize,
}

fn draw_cost(kind: VehicleKind) -> Cost {
    let baked = authoritative_baked_vehicle(kind).expect("shipped bake");
    let static_tris = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();

    let (mut gear_tris, mut gear_instances) = (0, 0);
    if let Some(kin) = RunningGearKinematics::for_vehicle(kind) {
        let wheel = road_wheel_unit_mesh(&kin).triangle_count();
        let link = track_link_unit_mesh(&kin).triangle_count();
        let arm = swing_arm_unit_mesh(&kin).triangle_count();
        let idler = idler_unit_mesh(&kin).triangle_count();
        let sprocket = sprocket_unit_mesh(&kin).triangle_count();
        for placement in running_gear_placements(&kin, 0.0, 0.0) {
            gear_instances += 1;
            gear_tris += match placement.part {
                GearPart::RoadWheel => wheel,
                GearPart::Link => link,
                GearPart::SwingArm => arm,
                GearPart::Idler => idler,
                GearPart::Sprocket => sprocket,
                _ => 0,
            };
        }
    }
    Cost { static_tris, gear_tris, gear_instances }
}

#[test]
fn the_measured_draw_cost_of_every_vehicle_and_of_a_full_battle() {
    println!(
        "\n| vehicle | static tris | gear tris | gear draws | TOTAL | x{BATTLE_TANKS} (7v7) |"
    );
    println!("|---|---:|---:|---:|---:|---:|");
    let (mut lightest, mut heaviest) = (usize::MAX, 0usize);
    for kind in VehicleKind::PLAYABLE {
        let cost = draw_cost(kind);
        let total = cost.static_tris + cost.gear_tris;
        lightest = lightest.min(total);
        heaviest = heaviest.max(total);
        println!(
            "| `{kind:?}` | {} | {} | {} | **{total}** | {} |",
            cost.static_tris,
            cost.gear_tris,
            cost.gear_instances,
            total * BATTLE_TANKS
        );
    }
    println!(
        "\nLightest {lightest}, heaviest {heaviest} ({:.1}x). A 7v7 of the heaviest draws {} \
         vehicle triangles.\n",
        heaviest as f32 / lightest.max(1) as f32,
        heaviest * BATTLE_TANKS
    );

    // The one thing worth asserting: the instanced gear is not a rounding error next to the bake
    // it is excluded from. If this ever stops holding, `VEHICLE_BUDGETS` alone would be a fair
    // description of a vehicle's cost and this file could go.
    let gear_matters = VehicleKind::PLAYABLE.into_iter().any(|kind| {
        let cost = draw_cost(kind);
        cost.gear_tris > cost.static_tris / 4
    });
    assert!(
        gear_matters,
        "no vehicle's instanced running gear reaches a quarter of its static bake — the budget \
         table would then describe the real cost on its own"
    );
}
