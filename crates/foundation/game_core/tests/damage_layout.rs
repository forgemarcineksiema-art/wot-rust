use game_core::{DamageLayout, ModuleSlot, VehicleKind};
use glam::Vec3;

#[test]
fn damage_layout_selects_the_highest_priority_matching_module() {
    let layout = DamageLayout::t54_1951();

    assert_eq!(layout.impacted_module(true, Vec3::new(0.0, 0.25, -1.8)), Some(ModuleSlot::Engine));
    assert_eq!(layout.impacted_module(false, Vec3::new(0.0, 0.25, -1.8)), None);
}

#[test]
fn damage_layout_ignores_hits_outside_all_module_volumes() {
    assert_eq!(DamageLayout::t54_1951().impacted_module(true, Vec3::new(0.0, 0.0, -2.9)), None);
}

#[test]
fn t54_damage_layout_fits_the_current_blueprint_hitbox() {
    assert!(DamageLayout::t54_1951().fits_within(VehicleKind::T54_1951.spec().hitbox));
}

#[test]
fn internal_path_hits_t54_modules_nearest_first() {
    let hits = DamageLayout::t54_1951().intersections(
        true,
        Vec3::new(0.0, 0.2, 3.2),
        Vec3::new(0.0, 0.2, -3.2),
    );
    let slots: Vec<_> = hits.iter().map(|hit| hit.slot).collect();
    assert_eq!(slots, vec![ModuleSlot::AmmoRack, ModuleSlot::Engine]);
    assert!(hits.iter().all(|hit| hit.path_length_m > 0.0));
}

#[test]
fn glacis_air_no_longer_magically_maps_to_the_gun() {
    assert_eq!(DamageLayout::t54_1951().impacted_module(true, Vec3::new(0.0, 0.2, 2.7)), None);
}
