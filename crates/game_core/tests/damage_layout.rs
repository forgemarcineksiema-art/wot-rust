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
    assert_eq!(DamageLayout::t54_1951().impacted_module(true, Vec3::new(0.0, 0.0, 2.9)), None);
}

#[test]
fn t54_damage_layout_fits_the_current_blueprint_hitbox() {
    assert!(DamageLayout::t54_1951().fits_within(VehicleKind::T54_1951.spec().hitbox));
}
