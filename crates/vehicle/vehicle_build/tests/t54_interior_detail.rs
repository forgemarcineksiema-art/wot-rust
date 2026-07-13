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
        "turret_extinguisher",
    ] {
        let matches = parts_named(&description, name);
        assert!(!matches.is_empty(), "missing museum-detail assembly {name}");
        assert!(matches.iter().all(|part| part.submesh == SubmeshKind::Turret));
    }
}

#[test]
fn detailed_modules_do_not_leave_coarse_collision_boxes_in_the_picture() {
    let description = t54_description();
    let proxies = parts_named(&description, "damage_component");
    assert_eq!(proxies.len(), 2, "only the two not-yet-detailed fuel tanks keep proxies");
    assert_eq!(proxies[0].key.instance, 8);
    assert_eq!(proxies[1].key.instance, 9);
}
