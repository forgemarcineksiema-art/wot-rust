use game_core::{DamageComponentKind, DamageLayout, ModuleSlot, VehicleKind};
use glam::{Mat3, Vec3};

#[test]
fn damage_layout_selects_the_highest_priority_matching_module() {
    let layout = DamageLayout::t54_1951();

    assert_eq!(layout.impacted_module(true, Vec3::new(0.0, 0.25, -1.8)), Some(ModuleSlot::Engine));
    assert_eq!(layout.impacted_module(false, Vec3::new(0.0, 0.25, -1.8)), None);
}

#[test]
fn damage_layout_ignores_hits_outside_all_module_volumes() {
    assert_eq!(DamageLayout::t54_1951().impacted_module(true, Vec3::new(0.0, 0.4, 2.9)), None);
}

#[test]
fn t54_damage_layout_fits_the_current_blueprint_hitbox() {
    assert!(DamageLayout::t54_1951().fits_within(VehicleKind::T54_1951.spec().hitbox));
}

#[test]
fn internal_path_hits_t54_modules_nearest_first() {
    // Down the loader's side: bow skeleton rack, fuel cell, transmission — in that order.
    let hits = DamageLayout::t54_1951().intersections(
        true,
        Vec3::new(0.65, 0.2, 3.2),
        Vec3::new(0.65, 0.2, -3.2),
    );
    let kinds: Vec<_> = hits.iter().map(|hit| hit.kind).collect();
    assert_eq!(
        kinds,
        vec![
            DamageComponentKind::AmmunitionRack,
            DamageComponentKind::FuelTank,
            DamageComponentKind::Transmission,
        ]
    );
    assert!(hits.iter().all(|hit| hit.path_length_m > 0.0));
    assert!(hits.windows(2).all(|pair| pair[0].distance_t <= pair[1].distance_t));
}

#[test]
fn the_drivers_side_of_the_hull_carries_no_ammunition() {
    // The pre-reconstruction layout invented a big rack on the left hull. Period stowage puts
    // the racked rounds to the loader's side; a full left-side run must meet none.
    let hits = DamageLayout::t54_1951().intersections(
        true,
        Vec3::new(-0.6, 0.2, 3.2),
        Vec3::new(-0.6, 0.2, -3.2),
    );
    assert!(hits.iter().all(|hit| hit.kind != DamageComponentKind::AmmunitionRack));
    let kinds: Vec<_> = hits.iter().map(|hit| hit.kind).collect();
    assert_eq!(
        kinds,
        vec![
            DamageComponentKind::Radio,
            DamageComponentKind::FuelTank,
            DamageComponentKind::Engine,
            DamageComponentKind::Transmission,
        ]
    );
}

#[test]
fn turret_rear_ammunition_swings_with_the_turret() {
    let layout = DamageLayout::t54_1951();
    let spec = VehicleKind::T54_1951.spec();
    let pivot = spec.mounts.turret_ring.translation - Vec3::Y * spec.hitbox.center_y_m;
    // Crosswise through the turret rear at ready-rack height.
    let start = Vec3::new(-1.5, 0.70, -0.61);
    let end = Vec3::new(1.5, 0.70, -0.61);

    let neutral = layout.intersections_with_turret_pose(true, start, end, 0.0, pivot);
    assert!(neutral.iter().any(|hit| hit.kind == DamageComponentKind::AmmunitionRack));

    let stale =
        layout.intersections_with_turret_pose(true, start, end, std::f32::consts::FRAC_PI_2, pivot);
    assert!(stale.iter().all(|hit| hit.kind != DamageComponentKind::AmmunitionRack));
}

#[test]
fn two_physical_components_may_feed_the_same_legacy_slot() {
    let hits = DamageLayout::t54_1951().intersections(
        true,
        Vec3::new(-1.0, 0.0, -1.43),
        Vec3::new(1.0, 0.0, -1.43),
    );
    let engine_components: Vec<_> = hits
        .iter()
        .filter(|hit| hit.slot == ModuleSlot::Engine)
        .map(|hit| hit.component_id)
        .collect();
    assert!(engine_components.len() >= 3, "fuel tanks and engine must remain distinct");
}

#[test]
fn glacis_air_no_longer_magically_maps_to_the_gun() {
    assert_eq!(DamageLayout::t54_1951().impacted_module(true, Vec3::new(0.0, 0.2, 2.7)), None);
}

#[test]
fn turret_components_follow_the_live_turret_yaw() {
    let layout = DamageLayout::t54_1951();
    let spec = VehicleKind::T54_1951.spec();
    let pivot = spec.mounts.turret_ring.translation - Vec3::Y * spec.hitbox.center_y_m;
    let neutral_start = Vec3::new(0.0, 0.82, 1.35);
    let neutral_end = Vec3::new(0.0, 0.82, 0.05);

    let neutral_hits =
        layout.intersections_with_turret_pose(true, neutral_start, neutral_end, 0.0, pivot);
    assert!(neutral_hits.iter().any(|hit| hit.kind == DamageComponentKind::Breech));

    let quarter_turn = std::f32::consts::FRAC_PI_2;
    let stale_hits = layout.intersections_with_turret_pose(
        true,
        neutral_start,
        neutral_end,
        quarter_turn,
        pivot,
    );
    assert!(stale_hits.iter().all(|hit| hit.kind != DamageComponentKind::Breech));

    let pose = Mat3::from_rotation_y(quarter_turn);
    let rotated_start = pivot + pose * (neutral_start - pivot);
    let rotated_end = pivot + pose * (neutral_end - pivot);
    let rotated_hits = layout.intersections_with_turret_pose(
        true,
        rotated_start,
        rotated_end,
        quarter_turn,
        pivot,
    );
    assert!(rotated_hits.iter().any(|hit| hit.kind == DamageComponentKind::Breech));
}
