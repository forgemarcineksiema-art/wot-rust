//! THE MEASURED COST OF PUTTING THE FLEET ON SCREEN.
//!
//! `VEHICLE_BUDGETS` bounds the STATIC bake and says so plainly — "runtime running gear is
//! excluded: wheels, suspension, end wheels, and links are animated instances". That is correct
//! for what it guards, and it means the number everyone quotes for a vehicle is not the number
//! the GPU draws. Road wheels, swing arms, idler, sprocket and every shoe link are real
//! triangles, instanced per side, and nothing wrote their total down.
//!
//! This file writes it down — and since PR-21 it also GUARDS it. The measurement came first and
//! the ceiling came from the measurement (`GEAR_BUDGETS`), which is the right order; what changed
//! is that the number can no longer grow without someone saying so. It also holds the distance
//! tier to account: a `Far` mesh set that saves little is a second mesh set for nothing.
//!
//! It lives in `vehicle_forge`, not in `vehicle_geometry`, so it can resolve each vehicle through
//! [`authoritative_baked_vehicle`] — the mesh the game actually draws. Measured from
//! `bake_vehicle` instead, the T-54 row would report its unused procedural recipe (1.6k triangles)
//! rather than the hybrid the client ships (15.1k), and a cost table that quotes a mesh nobody
//! renders is worse than no table.

use game_core::VehicleKind;
use vehicle_forge::authoritative_baked_vehicle;
use vehicle_geometry::{
    GearDetail, GearPart, RunningGearKinematics, idler_unit_mesh, return_roller_unit_mesh,
    road_wheel_unit_mesh, running_gear_placements, sprocket_unit_mesh, swing_arm_unit_mesh,
    track_link_unit_mesh,
};
use vehicle_recipes::{FAR_MUST_SAVE_FRACTION, GEAR_BUDGETS};

/// A 7v7 battle: the fleet size the game is designed around.
const BATTLE_TANKS: usize = 14;

struct Cost {
    static_tris: usize,
    gear_tris: usize,
    far_gear_tris: usize,
    gear_instances: usize,
}

/// What the gear costs to draw at one detail tier: every instance resolved to its unit mesh.
fn gear_tris(kin: &RunningGearKinematics) -> usize {
    let wheel = road_wheel_unit_mesh(kin).triangle_count();
    let link = track_link_unit_mesh(kin).triangle_count();
    let arm = swing_arm_unit_mesh(kin).triangle_count();
    let idler = idler_unit_mesh(kin).triangle_count();
    let sprocket = sprocket_unit_mesh(kin).triangle_count();
    let roller = return_roller_unit_mesh(kin).triangle_count();
    running_gear_placements(kin, 0.0, 0.0)
        .iter()
        .map(|placement| match placement.part {
            GearPart::RoadWheel => wheel,
            GearPart::Link => link,
            // The left arm is the right one mirrored: same triangle count by construction.
            GearPart::SwingArm | GearPart::SwingArmLeft => arm,
            GearPart::Idler => idler,
            GearPart::Sprocket => sprocket,
            GearPart::ReturnRoller => roller,
        })
        .sum()
}

fn draw_cost(kind: VehicleKind) -> Cost {
    let baked = authoritative_baked_vehicle(kind).expect("shipped bake");
    let static_tris = baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum();

    let (mut near, mut far, mut instances) = (0, 0, 0);
    if let Some(kin) = RunningGearKinematics::for_vehicle(kind) {
        near = gear_tris(&kin);
        far = gear_tris(&kin.at_detail(GearDetail::Far));
        instances = running_gear_placements(&kin, 0.0, 0.0).len();
    }
    Cost { static_tris, gear_tris: near, far_gear_tris: far, gear_instances: instances }
}

#[test]
fn the_measured_draw_cost_of_every_vehicle_and_of_a_full_battle() {
    println!(
        "\n| vehicle | static | gear near | gear far | saved | draws | TOTAL | x{BATTLE_TANKS} |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|");
    let (mut lightest, mut heaviest) = (usize::MAX, 0usize);
    for kind in VehicleKind::PLAYABLE {
        let cost = draw_cost(kind);
        let total = cost.static_tris + cost.gear_tris;
        lightest = lightest.min(total);
        heaviest = heaviest.max(total);
        let saved = if cost.gear_tris == 0 {
            0.0
        } else {
            1.0 - cost.far_gear_tris as f32 / cost.gear_tris as f32
        };
        println!(
            "| `{kind:?}` | {} | {} | {} | {:.0}% | {} | **{total}** | {} |",
            cost.static_tris,
            cost.gear_tris,
            cost.far_gear_tris,
            saved * 100.0,
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

/// The ceiling the table above earned. Every vehicle stays inside `GEAR_BUDGETS` at both tiers,
/// and the distant tier actually saves what it exists to save.
#[test]
fn every_vehicles_running_gear_stays_inside_its_budget() {
    let mut gears_budgeted = 0;
    for kind in VehicleKind::PLAYABLE {
        let cost = draw_cost(kind);
        if cost.gear_tris == 0 {
            continue;
        }
        gears_budgeted += 1;
        assert!(
            cost.gear_tris <= GEAR_BUDGETS.near_tri_max,
            "{kind:?}: {} gear triangles up close, past the {} ceiling — the running gear is the \
             largest body of geometry on the vehicle and it is now spent deliberately",
            cost.gear_tris,
            GEAR_BUDGETS.near_tri_max
        );
        assert!(
            cost.far_gear_tris <= GEAR_BUDGETS.far_tri_max,
            "{kind:?}: {} gear triangles at range, past the {} ceiling",
            cost.far_gear_tris,
            GEAR_BUDGETS.far_tri_max
        );
        assert!(
            cost.gear_instances <= GEAR_BUDGETS.instances_max,
            "{kind:?}: {} gear draw instances, past the {} ceiling",
            cost.gear_instances,
            GEAR_BUDGETS.instances_max
        );

        let saved = 1.0 - cost.far_gear_tris as f32 / cost.gear_tris as f32;
        assert!(
            saved >= FAR_MUST_SAVE_FRACTION,
            "{kind:?}: the distant tier saves only {:.0}% ({} -> {}) — a second mesh set has to \
             earn the memory and the switch it costs",
            saved * 100.0,
            cost.gear_tris,
            cost.far_gear_tris
        );

        // The tier changes MESHES, never placements: a tank at range stands in the same place, on
        // the same number of wheels, as one up close.
        let kin = RunningGearKinematics::for_vehicle(kind).expect("gear");
        let near = running_gear_placements(&kin, 0.0, 0.0);
        let far = running_gear_placements(&kin.at_detail(GearDetail::Far), 0.0, 0.0);
        assert_eq!(near.len(), far.len(), "{kind:?}: the detail tier moved the draw list");
        for (a, b) in near.iter().zip(far.iter()) {
            assert_eq!(a.part, b.part);
            assert!(
                (a.transform.w_axis - b.transform.w_axis).length() < 1.0e-5,
                "{kind:?}: a {:?} sits somewhere else at range",
                a.part
            );
        }
    }
    assert_eq!(
        gears_budgeted,
        VehicleKind::PLAYABLE.len(),
        "every playable vehicle draws running gear, so every one must be held to the budget"
    );
}
