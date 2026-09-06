use game_core::VehicleKind;
use vehicle_forge::{BakeProfile, TankCompileError, TankCompileRequest, compile_tank};

#[test]
fn compiling_t54_loadout_keeps_combat_shape_and_artifact_in_sync() {
    let kind = VehicleKind::T54_1951;
    let compiled = compile_tank(TankCompileRequest {
        vehicle: kind,
        modules: kind.default_loadout(),
        profile: BakeProfile::Lod0,
    })
    .expect("stock T-54 compiles");

    assert_eq!(compiled.spec.kind, kind);
    assert!(compiled.spec.damage_layout.fits_within(compiled.spec.hitbox));
    assert_eq!(compiled.source_hash, compiled.artifact.manifest().source_hash());
    assert_eq!(compiled.mounts, *compiled.baked_vehicle.mounts());
    assert_eq!(compiled.spec.mounts, compiled.mounts);
}

/// The T-54's two guns share one tube (see `both_d10_barrels_are_the_same_tube...`), so their
/// SILHOUETTES are identical by design — what must still differ is the gameplay spec. A vehicle
/// whose alternates genuinely reshape the mesh is covered by
/// `a_swapped_gun_module_reshapes_a_non_t54_vehicle` (the Jagdtiger's 8.8 vs 12.8 cm).
#[test]
fn changing_the_t54_gun_changes_the_spec_but_not_the_shared_tube() {
    let kind = VehicleKind::T54_1951;
    let stock = kind.default_loadout();
    let mut alternate = stock.clone();
    alternate.gun = kind.gun_options().into_iter().last().unwrap();
    let stock = compile_tank(TankCompileRequest {
        vehicle: kind,
        modules: stock,
        profile: BakeProfile::Lod0,
    })
    .unwrap();
    let alternate = compile_tank(TankCompileRequest {
        vehicle: kind,
        modules: alternate,
        profile: BakeProfile::Lod0,
    })
    .unwrap();

    assert_ne!(stock.spec.gun, alternate.spec.gun, "the installed gun module differs");
    assert_eq!(
        stock.mounts.muzzle.translation, alternate.mounts.muzzle.translation,
        "the D-10 variants share one physical tube — the muzzle must not move"
    );
}

#[test]
fn compiler_reports_all_loadout_validation_errors_together() {
    let kind = VehicleKind::T54_1951;
    let mut modules = kind.default_loadout();
    modules.gun.spec.shell.caliber_mm = 128.0;
    modules.engine.mass_kg = 99_000.0;

    let error =
        compile_tank(TankCompileRequest { vehicle: kind, modules, profile: BakeProfile::Lod0 })
            .unwrap_err();

    let TankCompileError::Validation(errors) = error else {
        panic!("expected grouped validation errors");
    };
    assert_eq!(errors.len(), 2);
}
