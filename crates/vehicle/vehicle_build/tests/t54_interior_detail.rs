use game_core::{DamageComponentId, DamageLayout, DamageShape, VehicleKind};
use glam::Vec3;
use vehicle_build::{VehicleDescription, t54_description};
use vehicle_geometry::SubmeshKind;

fn parts_named<'a>(
    description: &'a VehicleDescription,
    name: &'static str,
) -> Vec<&'a vehicle_build::VehiclePart> {
    description.parts.iter().filter(|part| part.key.name == name).collect()
}

#[test]
fn t54_front_hull_rack_exposes_twenty_individual_rounds() {
    let description = t54_description();
    let rounds = parts_named(&description, "bow_rack_round");
    assert_eq!(rounds.len(), 20, "the T-54 front skeleton rack carries twenty rounds");
    assert!(rounds.iter().all(|part| part.submesh == SubmeshKind::Hull));

    let mut instances: Vec<_> = rounds.iter().map(|part| part.key.instance).collect();
    instances.sort_unstable();
    assert_eq!(instances, (0..20).collect::<Vec<_>>());
}

#[test]
fn t54_1951_turret_rear_rounds_alternate_tip_to_tail() {
    let description = t54_description();
    let rounds = parts_named(&description, "turret_rear_round");
    assert_eq!(rounds.len(), 5, "the 1951 turret rear has five crosswise rounds");
    assert!(rounds.iter().all(|part| part.submesh == SubmeshKind::Turret));

    let centers_x: Vec<_> = rounds
        .iter()
        .map(|part| {
            let bounds = part.mesh().bounds().expect("round bounds");
            (bounds.min.x + bounds.max.x) * 0.5
        })
        .collect();
    assert!(centers_x[0] < 0.0 && centers_x[1] > 0.0 && centers_x[2] < 0.0);
}

#[test]
fn t54_loader_stowage_keeps_hull_and_turret_frames_separate() {
    let description = t54_description();
    let turret = parts_named(&description, "loader_wall_round");
    let hull = parts_named(&description, "loader_shin_round");
    assert_eq!(turret.len(), 2);
    assert_eq!(hull.len(), 4);
    assert!(turret.iter().all(|part| part.submesh == SubmeshKind::Turret));
    assert!(hull.iter().all(|part| part.submesh == SubmeshKind::Hull));
}

#[test]
fn t54_close_view_has_recognizable_fighting_compartment_assemblies() {
    let description = t54_description();
    for name in [
        "d10_breech_block",
        "d10_breech_ring",
        "d10_cradle_bridge",
        "d10_recoil_cylinder",
        "sg43_coax_receiver",
        "sg43_coax_barrel",
        "tsh2_sight_body",
        "tsh2_eyepiece",
        "radio_control_face",
        "turret_drive_housing",
        "turret_drive_gearbox",
        "turret_extinguisher",
    ] {
        let matches = parts_named(&description, name);
        assert!(!matches.is_empty(), "missing museum-detail assembly {name}");
        assert!(matches.iter().all(|part| part.submesh == SubmeshKind::Turret));
    }
}

#[test]
fn visible_weapon_and_driveline_parts_share_damage_layout_anchors() {
    let description = t54_description();
    let layout = DamageLayout::t54_1951();
    let center_y = VehicleKind::T54_1951.spec().hitbox.center_y_m;

    let breech_center = part_center(parts_named(&description, "d10_breech_ring")[0]);
    let (breech_min, breech_max) = component_obb_bounds(&layout, DamageComponentId(1), center_y);
    assert!(point_within(breech_center, breech_min, breech_max));

    let recoil_parts = parts_named(&description, "d10_recoil_cylinder");
    for (id, part) in [DamageComponentId(2), DamageComponentId(3)].into_iter().zip(recoil_parts) {
        let expected = component_center(&layout, id) + Vec3::Y * center_y;
        assert!((part_center(part) - expected).length() < 1.0e-4);
    }

    let drive = parts_named(&description, "turret_drive_housing");
    let expected = component_center(&layout, DamageComponentId(4)) + Vec3::Y * center_y;
    assert!((part_center(drive[0]) - expected).length() < 1.0e-4);
}

#[test]
fn racked_ammunition_rounds_sit_inside_their_authoritative_rack_volumes() {
    // Each racked group derives from its own AmmunitionRack component: bow skeleton rack (5),
    // turret-rear ready rounds (6), loader's shin rounds (16). The loose wall/bulkhead singles
    // are visual-only until the v26 component mask widens.
    let description = t54_description();
    let layout = DamageLayout::t54_1951();
    let center_y = VehicleKind::T54_1951.spec().hitbox.center_y_m;

    for (name, id) in [("bow_rack_round", 5), ("turret_rear_round", 6), ("loader_shin_round", 16)] {
        let (min, max) = component_obb_bounds(&layout, DamageComponentId(id), center_y);
        let rounds = parts_named(&description, name);
        assert!(!rounds.is_empty(), "missing ammunition family {name}");
        for part in rounds {
            assert!(
                point_within(part_center(part), min, max),
                "{name} instance {} escaped rack volume {id}",
                part.key.instance
            );
        }
    }
}

fn part_center(part: &vehicle_build::VehiclePart) -> Vec3 {
    let bounds = part.mesh().bounds().expect("part bounds");
    (bounds.min + bounds.max) * 0.5
}

fn component_center(layout: &DamageLayout, id: DamageComponentId) -> Vec3 {
    let component = layout.components().iter().find(|component| component.id == id).unwrap();
    match &component.shape {
        DamageShape::Obb { center, .. } | DamageShape::Cylinder { center, .. } => *center,
        _ => panic!("component {id:?} does not expose a center"),
    }
}

fn component_obb_bounds(
    layout: &DamageLayout,
    id: DamageComponentId,
    center_y: f32,
) -> (Vec3, Vec3) {
    let component = layout.components().iter().find(|component| component.id == id).unwrap();
    match &component.shape {
        DamageShape::Obb { center, half_extents, .. } => {
            let visual_center = *center + Vec3::Y * center_y;
            (visual_center - *half_extents, visual_center + *half_extents)
        }
        _ => panic!("component {id:?} is not an OBB"),
    }
}

fn point_within(point: Vec3, min: Vec3, max: Vec3) -> bool {
    point.cmpge(min - Vec3::splat(1.0e-4)).all() && point.cmple(max + Vec3::splat(1.0e-4)).all()
}

#[test]
fn detailed_modules_do_not_leave_coarse_collision_boxes_in_the_picture() {
    let description = t54_description();
    let proxies = parts_named(&description, "damage_component");
    assert_eq!(proxies.len(), 2, "only the two not-yet-detailed fuel tanks keep proxies");
    assert_eq!(proxies[0].key.instance, 8);
    assert_eq!(proxies[1].key.instance, 9);
}
