use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::MEDIUM_LOD0_TRI_BUDGET;
use vehicle_geometry::SubmeshKind;

#[test]
fn t54_closeup_budget_reserves_detail_for_the_cast_turret() {
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");

    assert_eq!(MEDIUM_LOD0_TRI_BUDGET, 22_000);
    assert_eq!(blueprint.hybrid().expect("hybrid visual").turret.budget, 12_000);
}

#[test]
fn t54_hybrid_keeps_a_low_wide_turret_and_headroom_for_garage_detail() {
    let baked = vehicle_build::t54_description().build();
    let total: usize = baked.submeshes().iter().map(|submesh| submesh.mesh.triangle_count()).sum();
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret");
    let bounds = turret.mesh.bounds().expect("turret bounds");

    assert!(total < MEDIUM_LOD0_TRI_BUDGET);
    assert!(turret.mesh.triangle_count() < 12_000);
    assert!(bounds.max.x - bounds.min.x > bounds.max.y - bounds.min.y);
}
