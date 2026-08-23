use game_core::{
    ArmorFacing, ArmorZone, ShellSpec, TankSpec, VehicleBlueprint, VehicleKind, known_tank_specs,
    resolve_penetration, vehicle_armor_volumes,
};

#[test]
fn is3_spec_matches_core_historical_shape() {
    let tank = VehicleKind::IS3.spec();

    assert_eq!(tank.name, "IS-3");
    assert!((tank.mass_kg - 46_000.0).abs() < 1_000.0);
    assert!((tank.engine_power_kw - 382.0).abs() < 10.0);
    assert!((tank.max_forward_speed_mps - 11.1).abs() < 0.2);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::HullFront), 110.0);
    assert_eq!(tank.hull.nominal_thickness_mm(ArmorFacing::TurretFront), 250.0);
    assert_eq!(tank.gun.name, "122 mm D-25T");
    assert_eq!(tank.gun.shell.caliber_mm, 122.0);
    assert!(tank.gun.shell.damage_hp > VehicleKind::T54_1951.spec().gun.shell.damage_hp);
}

#[test]
fn is3_is_available_in_known_specs() {
    let names: Vec<_> = known_tank_specs().into_iter().map(|spec| spec.name).collect();
    assert!(names.contains(&"IS-3".to_string()));
}

#[test]
fn is3_is_blueprint_born_with_pike_armor_volumes() {
    // The single-source chain holds from day one: blueprint drives hitbox, mounts, and the
    // armor volumes — and the bow is a real PIKE (two swept glacis planes, not one).
    let blueprint = VehicleBlueprint::for_vehicle(VehicleKind::IS3).expect("blueprint");
    assert_eq!(blueprint.hitbox(), game_core::HitboxProfile::for_vehicle(VehicleKind::IS3));
    assert!(blueprint.hull.pike_sweep_deg > 0.0);

    let volumes = vehicle_armor_volumes(VehicleKind::IS3).expect("armor volumes");
    let pike_headings: Vec<f32> = volumes.hull[0]
        .planes
        .iter()
        .filter(|plane| plane.zone == ArmorZone::UpperGlacis)
        .map(|plane| plane.normal.x.atan2(plane.normal.z).to_degrees())
        .collect();
    assert_eq!(pike_headings.len(), 2, "a pike bow is two glacis planes");
    assert!(
        (pike_headings[0] + pike_headings[1]).abs() < 1.0e-3,
        "the pike faces mirror about the ridge: {pike_headings:?}"
    );
    assert!(
        (pike_headings[0].abs() - blueprint.hull.pike_sweep_deg).abs() < 0.5,
        "the sweep is the blueprint sweep: {pike_headings:?}"
    );
}

#[test]
fn the_d25t_cannot_open_the_is3_pike_head_on_but_the_d10_side_falls() {
    // Tier-internal sanity: the heavy's own 122 mm bounces off a squarely-faced pike (the
    // compound ~64° true angle beats 175 mm of penetration), while the 90 mm tub side behind
    // the tracks stays honest against the mediums' 100 mm guns.
    let is3 = VehicleKind::IS3.spec();
    let pike_true_angle = {
        let sweep = VehicleBlueprint::for_vehicle(VehicleKind::IS3)
            .expect("blueprint")
            .hull
            .pike_sweep_deg
            .to_radians();
        let slope = is3.hull.facet(ArmorFacing::HullFront).slope_degrees.to_radians();
        (slope.cos() * sweep.cos()).acos().to_degrees()
    };
    let d25t = ShellSpec::armor_piercing(122.0, 795.0, 175.0, 390);
    let head_on = resolve_penetration(&d25t, &is3.hull, ArmorFacing::HullFront, pike_true_angle);
    assert!(!head_on.penetrated, "a square pike face turns the era's own 122 mm");

    let d10 = TankSpec::t54_1951().gun.shell;
    let side = resolve_penetration(&d10, &is3.hull, ArmorFacing::HullSide, 0.0);
    assert!(side.penetrated, "the flat tub side stays honest against the mediums");
}
