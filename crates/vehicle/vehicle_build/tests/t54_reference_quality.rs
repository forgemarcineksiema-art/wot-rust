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

/// What the OUTSIDE of the gun mount is allowed to be. This used to measure the whole gun submesh
/// and cap it at a "trim pig's head" — a set of numbers describing an external ball mantlet, which
/// the dossier says this vehicle does not have. The mantlet is inside now, so measuring the gun
/// submesh measures the wrong thing: most of it is behind the turret face.
///
/// The question worth locking is what a viewer sees ahead of the casting: CANVAS — the panel over
/// the window, out to its own half-diagonal — and the tube. Every piece of steel other than the
/// barrel stays behind the face, inside the window; steel standing ahead is the ball mantlet
/// creeping back.
#[test]
fn the_visible_gun_mount_is_no_wider_than_its_canvas_cover() {
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let hybrid = blueprint.hybrid().expect("hybrid visual");
    let casting = vehicle_build::t54_turret_loft(&hybrid.turret_loft);
    let face = casting.bounds().expect("casting bounds").max.z;
    let trunnion_y = blueprint.gun.trunnion_y;

    let baked = vehicle_build::t54_description().build();
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let radius_ahead = |wanted: bool| {
        gun.vertices()
            .iter()
            .filter(|v| v.position.z > face && (v.material == MaterialRole::Canvas) == wanted)
            .map(|v| v.position.x.hypot(v.position.y - trunnion_y))
            .fold(0.0_f32, f32::max)
    };
    // The panel's fastened rectangle is the measured 0.45 x 0.37 pillow; its half-diagonal is
    // ~0.29 and the fabric never stands wider than what it is fastened over.
    let fabric = radius_ahead(true);
    assert!(
        fabric <= 0.34,
        "ahead of the turret face the widest thing is the fastened panel, got fabric at {fabric:.3}"
    );
    assert!(fabric > 0.22, "and it IS the fastened panel, not a bare boot: {fabric:.3}");
    let steel = radius_ahead(false);
    assert!(
        steel <= 0.13,
        "the only steel ahead of the face is the tube — anything wider is the ball mantlet \
         creeping back, got {steel:.3}"
    );
}

/// K2, at the source. The mantlet profile must close at BOTH ends. Written as a sleeve — first
/// station 0.13, last station 0.10, neither of them zero — the revolve produced a tube with two
/// open rims standing in mid-air, and nothing downstream objected: the gun submesh is held to
/// `OPEN_OR_CLOSED`, which is exactly the contract that cannot see this.
#[test]
fn the_mantlet_profile_closes_at_both_ends() {
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).expect("T-54 blueprint");
    let gun = &blueprint.hybrid().expect("hybrid visual").gun;
    let profile = gun.mantlet_profile;
    assert_eq!(profile[0].1, 0.0, "the mantlet has a back");
    assert_eq!(profile[profile.len() - 1].1, 0.0, "and a front");

    let depth = profile[profile.len() - 1].0 - profile[0].0;
    assert!(
        (0.15..=0.40).contains(&depth),
        "an internal mantlet is a compact cast body, not a sleeve reaching out of the turret: \
         depth {depth:.3}"
    );
    let max_radius = profile.iter().map(|(_, radius)| *radius).fold(0.0, f32::max);
    assert!(
        max_radius * gun.mantlet_scale.x <= 0.30,
        "and it stays subordinate to the cheeks: radius {max_radius:.3}, scale {:.3}",
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
