use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{MaterialRole, RunningGearKinematics, SubmeshKind, road_wheel_unit_mesh};
use vehicle_recipes::bake_vehicle;

#[test]
fn t54_bake_has_mount_aware_submeshes_and_stable_output() {
    let first = bake_vehicle(VehicleKind::T54_1951).expect("T-54 recipe should bake");
    let second = bake_vehicle(VehicleKind::T54_1951).expect("T-54 recipe should bake repeatedly");

    assert_eq!(first.kind(), VehicleKind::T54_1951);
    assert_eq!(first.deterministic_hash(), second.deterministic_hash());

    let hull = first.submesh(SubmeshKind::Hull).expect("hull submesh");
    let turret = first.submesh(SubmeshKind::Turret).expect("turret submesh");
    let gun = first.submesh(SubmeshKind::Gun).expect("gun submesh");

    assert!(hull.mesh.triangle_count() >= 80, "hull should be richer than box tracks");

    // Road wheels are now animated running gear rather than baked into the hull. Their rubber
    // geometry and outboard placement are read from the kinematics the renderer instances from.
    let kin = RunningGearKinematics::for_vehicle(VehicleKind::T54_1951).expect("T-54 running gear");
    assert!(kin.wheel_zs.len() >= 5, "T-54 should carry its road-wheel line");
    let wheel_rubber = road_wheel_unit_mesh(&kin)
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::Rubber)
        .count();
    assert!(wheel_rubber >= 24, "road-wheel disc should be rubber geometry");
    assert!(
        kin.wheel_x + kin.wheel_half_width >= 1.50,
        "road wheels should sit on the visible outside face of the track run"
    );
    assert!(turret.mesh.triangle_count() >= 48, "turret should be rounded enough to read");
    assert!(gun.mesh.triangle_count() >= 24, "gun should be a revolved barrel");

    assert!(first.mounts().turret_ring.translation.is_finite());
    assert!(first.mounts().gun_trunnion.translation.is_finite());
    assert!(first.mounts().muzzle.translation.is_finite());
}

/// Ambient contact: a tank is darker down where its hull meets its running gear than up on the
/// deck, and the bake is what puts that difference in before any light hits it.
///
/// The threshold used to be an absolute `<= 0.72`, and it was reached by an accident. The legacy
/// hull's lower tub is an EXTRUSION — it has vertices only at its end sections, none along the
/// track run — so nothing in the running-gear region can be measured on it at all. What actually
/// carried that 0.72 was the running-gear recess cavity overshooting the belt by 0.42 m in Z and
/// catching the NOSE PLATE, which is not a recess and has no business being shaded like one.
/// Shortening the belt to its documented 90 x 137 mm (PR-18) moved the band off the nose and the
/// number moved with it.
///
/// So the assertion is the CONTRAST, which is the thing the pass exists to create, and it is
/// measured where the mesh actually has metal.
#[test]
fn t54_surface_shading_darkens_the_hull_bottom_against_its_deck() {
    let baked = bake_vehicle(VehicleKind::T54_1951).expect("T-54 recipe should bake");
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh");

    let hull_bottom = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.position.y < 0.45)
        .map(|vertex| vertex.surface_shade)
        .fold(1.0_f32, f32::min);
    let upper_armor = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::RolledArmor && vertex.position.y > 1.0)
        .map(|vertex| vertex.surface_shade)
        .fold(0.0_f32, f32::max);

    assert!(upper_armor >= 0.95, "the deck reads as lit armour, got {upper_armor:.3}");
    assert!(
        upper_armor - hull_bottom >= 0.15,
        "the hull bottom must read as shaded against the deck: {hull_bottom:.3} vs          {upper_armor:.3}"
    );
}

#[test]
fn t54_body_fits_and_fills_gameplay_hitbox_but_gun_can_protrude() {
    let baked = bake_vehicle(VehicleKind::T54_1951).expect("T-54 recipe should bake");
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let body = baked.body_bounds().expect("body bounds");
    let gun = baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh.bounds().unwrap();

    let top = hitbox.center_y_m + hitbox.half_height_m;
    let floor = hitbox.center_y_m - hitbox.half_height_m;

    assert!(body.min.x >= -hitbox.half_width_m - 0.05);
    assert!(body.max.x <= hitbox.half_width_m + 0.05);
    assert!(body.min.z >= -hitbox.half_length_m - 0.05);
    assert!(body.max.z <= hitbox.half_length_m + 0.05);
    assert!(body.min.y >= floor - 0.15);
    assert!(body.max.y <= top + 0.05);

    assert!(body.max.x >= hitbox.half_width_m * 0.88);
    assert!(body.max.z >= hitbox.half_length_m * 0.88);
    assert!(body.max.y >= top - 0.30);
    assert!(gun.max.z > hitbox.half_length_m, "barrel should protrude past the hitbox");
}

/// W4 F5.ii: a blueprint that AUTHORS a gun part bakes the generic revolved group — provable
/// from the mesh itself, because the group's signature is the bore: a ring of vertices at the
/// gun's calibre radius at the muzzle face, where the legacy plan drew near-solid steel with a
/// dimple. The dispatch reads data, not identity: this test asks the blueprint first, so the
/// day another vehicle authors a part it is covered with no edit here.
#[test]
fn an_authored_gun_part_bakes_the_bore_honest_group() {
    let mut covered = 0;
    for kind in VehicleKind::PLAYABLE {
        let Some(blueprint) = game_core::VehicleBlueprint::for_vehicle(kind) else { continue };
        let Some(gun) = blueprint.visual_detail().and_then(|d| d.gun.as_ref().copied()) else {
            continue;
        };
        covered += 1;
        let baked = bake_vehicle(kind).expect("recipe bakes");
        let mesh = &baked.submesh(SubmeshKind::Gun).expect("gun submesh").mesh;
        let muzzle_z = blueprint.gun.muzzle_z;
        let axis_y = blueprint.gun.trunnion_y;
        let bore_ring = mesh
            .vertices()
            .iter()
            .filter(|v| (v.position.z - muzzle_z).abs() < 1.0e-3)
            .filter(|v| {
                let r = v.position.x.hypot(v.position.y - axis_y);
                (r - gun.bore_radius).abs() < 1.0e-3
            })
            .count();
        assert!(
            bore_ring >= gun.barrel_segments,
            "{kind:?}: the muzzle face must carry a bore ring at the calibre radius \
             (found {bore_ring} vertices) — the authored part bakes the bore-honest group"
        );
    }
    assert!(covered >= 1, "the benchmark authors a gun part, so this test must cover it");
}
