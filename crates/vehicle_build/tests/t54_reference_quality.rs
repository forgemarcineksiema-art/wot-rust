use game_core::{VehicleBlueprint, VehicleKind};
use vehicle_build::MEDIUM_LOD0_TRI_BUDGET;
use vehicle_geometry::{MaterialRole, SubmeshKind};

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

#[test]
fn t54_turret_has_the_flat_pancake_profile_of_the_1951_casting() {
    let bp = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("blueprint");
    let roof_y = bp.hybrid().expect("hybrid").turret_loft.stations.last().unwrap().y;
    let baked = vehicle_build::t54_description().build();
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret");

    // The pancake profile is a property of the cast shell up to the roof — measure that band, not the
    // proud roof fittings (cupola, DShK pedestal, periscopes) that legitimately stand above it.
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for v in turret.mesh.vertices().iter().filter(|v| v.position.y <= roof_y + 0.02) {
        min_x = min_x.min(v.position.x);
        max_x = max_x.max(v.position.x);
        min_y = min_y.min(v.position.y);
        max_y = max_y.max(v.position.y);
    }
    let (width, height) = (max_x - min_x, max_y - min_y);

    assert!(
        height / width <= 0.36,
        "T-54 turret casting must read as a low pancake, got height/width {:.3}",
        height / width
    );
}

#[test]
fn t54_turret_carries_a_loader_side_dshk_mount() {
    let baked = vehicle_build::t54_description().build();
    let turret = baked.submesh(SubmeshKind::Turret).expect("turret");
    let dshk_vertices = turret
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::BarrelSteel)
        .filter(|vertex| vertex.position.y > 1.85 && vertex.position.x > 0.15)
        .count();

    assert!(
        dshk_vertices >= 16,
        "T-54-3 needs a visible loader-side DShK mount on the turret roof"
    );
}
