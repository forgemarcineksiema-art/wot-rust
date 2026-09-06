use client::{
    CamoPattern, DecalFrame, DecalKind, EquipmentAnchor, HitDecal, MAX_HIT_DECALS,
    VehicleAssetCatalog, VehicleVariation, equipment_points, tank_vehicle_render_objects,
    tank_vehicle_render_objects_with_variation,
};
use game_core::{ModuleSlot, TankId, TeamId, VehicleKind};
use net::TankSnapshot;

#[test]
fn a_clean_vehicle_keeps_its_base_colour() {
    let clean = VehicleVariation::default();
    let base = [0.30, 0.40, 0.28];
    assert_eq!(clean.surface_tint(base), base);
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

fn decal(x: f32, kind: DecalKind) -> HitDecal {
    HitDecal {
        local_position: [x, 0.0, 0.0],
        local_normal: [1.0, 0.0, 0.0],
        local_tangent: [0.0, 1.0, 0.0],
        radius: 0.2,
        age_s: 0.0,
        kind,
        frame: DecalFrame::Hull,
        patch: None,
    }
}

#[test]
fn hit_decals_are_capped_and_every_mark_stays_for_the_battle() {
    let mut v = VehicleVariation::default();
    for i in 0..(MAX_HIT_DECALS + 4) {
        v.record_hit(decal(i as f32 * 0.1, DecalKind::Scuff));
    }
    assert_eq!(v.decals().len(), MAX_HIT_DECALS, "decal count must stay bounded");
    assert!((v.decals()[0].opacity() - 1.0).abs() < 1.0e-6, "fresh decal is fully opaque");

    // A battle later: every scuff is still there at full strength (Inny Poziom Z5, game-design
    // §13.1 — marks stay for the battle, the oldest merge into wear). Only the cap recycles.
    v.tick(900.0);
    assert_eq!(v.decals().len(), MAX_HIT_DECALS, "no mark fades away");
    assert!(v.decals().iter().all(|d| (d.opacity() - 1.0).abs() < 1.0e-6));
    v.record_hit(decal(9.0, DecalKind::Penetration));
    assert_eq!(v.decals().len(), MAX_HIT_DECALS, "the cap recycles the oldest");
    assert_eq!(v.decals().last().map(|d| d.kind), Some(DecalKind::Penetration));
}

#[test]
fn sync_from_snapshot_updates_damage_state_but_keeps_the_scars() {
    let mut v = VehicleVariation::default();
    v.record_hit(decal(0.0, DecalKind::Penetration));
    let mut snap = snapshot(VehicleKind::T54_1951);
    snap.destroyed_modules_mask = ModuleSlot::Engine.destroyed_mask_bit();

    v.sync_from_snapshot(&snap);

    assert!(v.module_destroyed(ModuleSlot::Engine), "damage state adopts the snapshot");
    assert_eq!(v.decals().len(), 1, "the accumulated scars survive the sync");
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

    // A LIVE tank never recolours with damage; only a wreck takes the burnt-out tint.
    let base = [0.5, 0.5, 0.5];
    let charred = VehicleVariation::wreck_tint(base);
    assert!(charred[0] < base[0] * 0.5, "a wreck is visibly burnt out");
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
    let snapshot = snapshot(VehicleKind::T54_1951);
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
        hull_pitch_rad: 0.0,
        hull_roll_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: vehicle.spec().gun.dispersion_mrad,
        module_hit_points: vehicle.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
        track_damage_mask: 0,
        track_hp: [game_core::TRACK_HP_MAX; 2],
        ammo_counts: game_core::AmmoLoadout::default().counts,
        selected_ammo: 0,
        spotted_by_teams_mask: 0,
        armor_breaches: Default::default(),
        track_break_t: [None, None],
        engine_fire: false,
        fuel_fire: false,
        rack_fire_remaining_s: None,
        crew_unconscious_mask: 0,
        crew_weakened_mask: 0,
        crew_down_remaining_s: Default::default(),
        hull_pitch_velocity_rad_s: 0.0,
        hull_roll_velocity_rad_s: 0.0,
    }
}
