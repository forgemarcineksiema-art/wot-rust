use client::{
    CamoPattern, DECAL_FADE_S, EquipmentAnchor, MAX_HIT_DECALS, VehicleAssetCatalog,
    VehicleVariation, equipment_points, tank_vehicle_render_objects,
    tank_vehicle_render_objects_with_variation,
};
use game_core::{ModuleSlot, TankId, TeamId, VehicleKind};
use net::TankSnapshot;

#[test]
fn a_clean_vehicle_keeps_its_base_colour() {
    let clean = VehicleVariation::default();
    let base = [0.30, 0.40, 0.28];
    assert_eq!(clean.surface_tint(base), base);
    assert_eq!(clean.module_tint(base, &[ModuleSlot::Engine]), base);
    assert!(!clean.tracks_broken());
    assert!(clean.decals().is_empty());
}

#[test]
fn weather_and_camo_overlays_shift_the_surface_tint() {
    let base = [0.30, 0.40, 0.28];
    assert_ne!(VehicleVariation::default().with_dirt(1.0).surface_tint(base), base);
    let snowy = VehicleVariation::default().with_snow(1.0).surface_tint(base);
    assert!(snowy[0] > base[0] && snowy[2] > base[2], "snow should brighten the hull");
    assert_ne!(VehicleVariation::default().with_camo(CamoPattern::Desert).surface_tint(base), base);
}

#[test]
fn hit_decals_are_capped_and_fade_over_time() {
    let mut v = VehicleVariation::default();
    for i in 0..(MAX_HIT_DECALS + 4) {
        v.record_hit([i as f32 * 0.1, 0.0, 0.0], 0.2);
    }
    assert_eq!(v.decals().len(), MAX_HIT_DECALS, "decal count must stay bounded");
    assert!((v.decals()[0].opacity() - 1.0).abs() < 1.0e-6, "fresh decal is fully opaque");

    v.tick(DECAL_FADE_S * 0.5);
    assert!(v.decals()[0].opacity() < 0.6, "decals fade as they age");
    v.tick(DECAL_FADE_S);
    assert!(v.decals().is_empty(), "fully faded decals are dropped");
}

#[test]
fn snapshot_drives_broken_tracks_and_module_damage() {
    let mut snapshot = snapshot(VehicleKind::T54_1951);
    snapshot.destroyed_modules_mask =
        ModuleSlot::Suspension.destroyed_mask_bit() | ModuleSlot::Engine.destroyed_mask_bit();
    let v = VehicleVariation::from_snapshot(&snapshot);

    assert!(v.tracks_broken(), "destroyed suspension reads as broken tracks");
    assert!(v.module_destroyed(ModuleSlot::Engine));
    assert!(!v.module_destroyed(ModuleSlot::Turret));

    // A dead engine darkens the hull submesh tint.
    let base = [0.5, 0.5, 0.5];
    let damaged = v.module_tint(base, &[ModuleSlot::Engine, ModuleSlot::Suspension]);
    assert!(damaged[0] < base[0], "destroyed module must darken its submesh");
}

#[test]
fn equipment_points_sit_on_the_baked_body() {
    let points = equipment_points(VehicleKind::T54_1951);
    assert!(points.iter().any(|p| p.name == "spare_track" && p.anchor == EquipmentAnchor::Hull));
    assert!(points.iter().any(|p| p.name == "antenna" && p.anchor == EquipmentAnchor::Turret));
    for point in &points {
        assert!(point.local_position.iter().all(|c| c.is_finite()));
    }
}

#[test]
fn variation_changes_rendered_tint_without_changing_geometry() {
    let mut catalog = VehicleAssetCatalog::default();
    let snapshot = snapshot(VehicleKind::T55A);
    let base = [0.30, 0.40, 0.28];

    let clean = tank_vehicle_render_objects(&mut catalog, &snapshot, base);
    let winter = tank_vehicle_render_objects_with_variation(
        &mut catalog,
        &snapshot,
        base,
        &VehicleVariation::default().with_camo(CamoPattern::Winter).with_snow(0.8),
    );

    assert_eq!(clean.len(), winter.len());
    for (a, b) in clean.iter().zip(&winter) {
        assert_eq!(a.mesh, b.mesh, "variation must reuse the same baked mesh");
        assert_ne!(a.tint, b.tint, "winter camo + snow must recolour the vehicle");
    }
}

fn snapshot(vehicle: VehicleKind) -> TankSnapshot {
    TankSnapshot {
        tank_id: TankId(7),
        team: TeamId(1),
        vehicle,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: vehicle.spec().gun.dispersion_mrad,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
    }
}
