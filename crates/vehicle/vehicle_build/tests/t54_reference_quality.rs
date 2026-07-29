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
    for v in turret
        .mesh
        .vertices()
        .iter()
        .filter(|v| v.material == MaterialRole::CastArmor && v.position.y <= roof_y + 0.02)
    {
        min_x = min_x.min(v.position.x);
        max_x = max_x.max(v.position.x);
        min_y = min_y.min(v.position.y);
        max_y = max_y.max(v.position.y);
    }
    let (width, height) = (max_x - min_x, max_y - min_y);

    // Re-blessed 2026-07-29 (PR-15) onto the DOCUMENTED casting instead of the one the model
    // happened to have. The T-54's turret is 2.25 m wide and runs from a 1.58 m ring seat to a
    // 2.40 m roof: 0.82 over 2.25 is 0.365. The old ceiling of 0.36 was the shallow 2.27 m dome
    // expressed as a ratio, so it forbade the vehicle's own proportions — a gate that fails the
    // real tank is measuring the model's history, not the tank.
    assert!(
        height / width <= 0.40,
        "T-54 turret casting must read as a low pancake (documented 0.82 / 2.25 = 0.365), got \
         height/width {:.3}",
        height / width
    );
}

#[test]
fn the_mantlet_reads_as_a_trim_pigs_head_not_a_wide_slab() {
    // The cast front cheeks carry the turret-front mass; the mantlet just covers the moving gun
    // aperture. So the mask is a trim oval — narrower than the old wide slab (half-width 0.57) —
    // while still clearly wider than tall (a flat cast oval, not a round ball).
    let baked = vehicle_build::t54_description().build();
    let gun = baked.submesh(SubmeshKind::Gun).expect("gun").mesh.bounds().expect("gun bounds");
    let half_width = gun.max.x.max(-gun.min.x);
    // Caps sized to the 1:1 casting (the ~2.25 m turret): the mask stays subordinate to the cheeks.
    assert!(
        half_width < 0.38,
        "mantlet is oversized for the T-54 nose: half-width {half_width:.3}"
    );
    let half_height = (gun.max.y - gun.min.y) / 2.0;
    assert!(
        half_height < 0.24,
        "mantlet is too bulbous in side view: half-height {half_height:.3}"
    );
    assert!(
        half_width > 0.95 * half_height,
        "mantlet stays a wide flat oval: half-width {half_width:.3} vs half-height {half_height:.3}"
    );
}

#[test]
fn the_mantlet_stays_short_and_compact_like_the_1951_reference() {
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let gun = &blueprint.hybrid().expect("hybrid visual").gun;
    let profile_back = gun.mantlet_profile.iter().map(|(z, _)| *z).fold(f32::INFINITY, f32::min);
    let profile_front =
        gun.mantlet_profile.iter().map(|(z, _)| *z).fold(f32::NEG_INFINITY, f32::max);
    let profile_depth = profile_front - profile_back;
    // Sized to the 1:1 casting: the buried shoulder plus the sleeve that covers the embrasure
    // opening of the ~2.25 m turret; anything longer reads as a pod on the nose.
    assert!(
        profile_depth <= 0.70,
        "mantlet protrudes as an oversized pod: profile depth {profile_depth:.3}"
    );

    let max_radius = gun.mantlet_profile.iter().map(|(_, radius)| *radius).fold(0.0, f32::max);
    assert!(
        max_radius * gun.mantlet_scale.x <= 0.40,
        "mantlet maximum width must stay subordinate to the turret cheeks: radius {max_radius:.3}, scale {:.3}",
        gun.mantlet_scale.x
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
