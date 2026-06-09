use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{MaterialRole, SubmeshKind, bake_vehicle};

#[test]
fn t55a_bake_has_mount_aware_submeshes_and_stable_output() {
    let first = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe should bake");
    let second = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe should bake repeatedly");

    assert_eq!(first.kind(), VehicleKind::T55A);
    assert_eq!(first.deterministic_hash(), second.deterministic_hash());

    let hull = first.submesh(SubmeshKind::Hull).expect("hull submesh");
    let turret = first.submesh(SubmeshKind::Turret).expect("turret submesh");
    let gun = first.submesh(SubmeshKind::Gun).expect("gun submesh");

    assert!(hull.mesh.triangle_count() >= 80, "hull should be richer than box tracks");
    assert!(
        hull.mesh
            .vertices()
            .iter()
            .filter(|vertex| vertex.material == MaterialRole::Rubber)
            .count()
            >= 144,
        "T-55A hull should include visible road-wheel geometry"
    );
    let outer_wheel_x = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == MaterialRole::Rubber)
        .map(|vertex| vertex.position.x.abs())
        .fold(0.0_f32, f32::max);
    assert!(
        outer_wheel_x >= 1.70,
        "road wheels should sit on the visible outside face of the track run"
    );
    assert!(turret.mesh.triangle_count() >= 48, "turret should be rounded enough to read");
    assert!(gun.mesh.triangle_count() >= 24, "gun should be a revolved barrel");

    assert!(first.mounts().turret_ring.translation.is_finite());
    assert!(first.mounts().gun_trunnion.translation.is_finite());
    assert!(first.mounts().muzzle.translation.is_finite());
}

#[test]
fn t55a_surface_shading_darkens_lower_running_gear() {
    let baked = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe should bake");
    let hull = baked.submesh(SubmeshKind::Hull).expect("hull submesh");

    let lower_running_gear = hull
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

    assert!(lower_running_gear <= 0.72);
    assert!(upper_armor >= 0.95);
}

#[test]
fn t55a_body_fits_and_fills_gameplay_hitbox_but_gun_can_protrude() {
    let baked = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe should bake");
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T55A);
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
