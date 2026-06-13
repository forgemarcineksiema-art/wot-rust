use client::{VehicleMeshCatalog, tank_render_objects, tank_scene_mesh};
use game_core::{ModuleSlot, TankId, TeamId, VehicleKind};
use glam::{Mat4, Vec3};
use net::TankSnapshot;
use vehicle_geometry::{SmoothingGroup, SubmeshKind, bake_vehicle};

const SG_RING: SmoothingGroup = SmoothingGroup(7);

/// Drift lock between the two render paths: the dynamic per-vertex mesh build and the cached
/// instanced objects must place every vertex identically for the same snapshot — including a
/// posed turret, pitched gun, and a casemate holding its yaw. If either path grows its own
/// transform math again, this turns red.
#[test]
fn dynamic_and_instanced_paths_agree_on_world_space_vertices() {
    for (kind, turret_yaw) in [(VehicleKind::T55A, 0.4), (VehicleKind::Jagdtiger, 1.2)] {
        let snapshot = TankSnapshot {
            tank_id: TankId(7),
            team: TeamId(1),
            vehicle: kind,
            position: [10.0, 2.0, 30.0],
            yaw_rad: 0.7,
            turret_yaw_rad: turret_yaw,
            turret_yaw_velocity_rad_s: 0.0,
            gun_pitch_rad: 0.12,
            hit_points: 900,
            reload_remaining_s: 0.0,
            aim_dispersion_mrad: kind.spec().gun.dispersion_mrad,
            module_hit_points: kind.spec().module_health.hit_points_by_slot(),
            destroyed_modules_mask: 0,
        };

        let (dynamic_vertices, _) = tank_scene_mesh(&snapshot);

        let mut catalog = VehicleMeshCatalog::default();
        let objects = tank_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
        let uploads = catalog.take_pending_meshes();
        let mut instanced_vertices = Vec::new();
        for object in &objects {
            let (_, mesh) = uploads
                .iter()
                .find(|(handle, _)| *handle == object.mesh)
                .expect("object mesh was uploaded");
            let transform = Mat4::from_cols_array_2d(&object.transform);
            for vertex in mesh.vertices() {
                instanced_vertices
                    .push(transform.transform_point3(Vec3::from_array(vertex.position)));
            }
        }

        assert_eq!(dynamic_vertices.len(), instanced_vertices.len(), "{kind:?} vertex counts");
        for (dynamic, instanced) in dynamic_vertices.iter().zip(&instanced_vertices) {
            let world = Vec3::from_array(dynamic.position);
            assert!(
                (world - *instanced).length() < 1.0e-4,
                "{kind:?} vertex drifted between render paths: {world:?} vs {instanced:?}"
            );
        }
    }
}

#[test]
fn t55a_render_objects_use_static_mesh_handles_for_hull_turret_and_gun() {
    let mut catalog = VehicleMeshCatalog::default();
    let snapshot = TankSnapshot {
        tank_id: TankId(7),
        team: TeamId(1),
        vehicle: VehicleKind::T55A,
        position: [10.0, 2.0, 30.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: VehicleKind::T55A.spec().gun.dispersion_mrad,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    };

    let objects = tank_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);

    assert_eq!(objects.len(), 3);
    assert!(objects.iter().all(|object| object.tank_id == Some(TankId(7))));
    assert_ne!(objects[0].mesh, objects[1].mesh);
    assert_ne!(objects[1].mesh, objects[2].mesh);
    assert!(objects.windows(2).all(|pair| pair[0].material == pair[1].material));

    let hull_pos = Mat4::from_cols_array_2d(&objects[0].transform).w_axis.truncate();
    let turret_pos = Mat4::from_cols_array_2d(&objects[1].transform).w_axis.truncate();
    let gun_pos = Mat4::from_cols_array_2d(&objects[2].transform).w_axis.truncate();

    assert_eq!(hull_pos, glam::Vec3::new(10.0, 2.0, 30.0));
    assert!(turret_pos.y > hull_pos.y, "turret should sit above hull");
    assert!(gun_pos.z > turret_pos.z, "gun trunnion should sit in front of turret ring");
    assert!(gun_pos.y >= hull_pos.y + 0.30, "gun barrel should clear the hull glacis");
}

#[test]
fn gun_mantlet_clears_glacis_and_sits_at_turret_height() {
    let vehicle = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe");
    let mounts = vehicle.mounts();
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
    let turret_bounds = turret.mesh.bounds().expect("turret bounds");

    assert!(
        mounts.gun_trunnion.translation.z >= turret_bounds.max.z - 0.05,
        "trunnion should sit at turret front (z={:.2}, turret_max_z={:.2})",
        mounts.gun_trunnion.translation.z,
        turret_bounds.max.z
    );
    assert!(
        mounts.gun_trunnion.translation.y >= turret_bounds.min.y
            && mounts.gun_trunnion.translation.y <= turret_bounds.max.y,
        "trunnion y={:.2} should stay within turret vertical bounds [{:.2}, {:.2}]",
        mounts.gun_trunnion.translation.y,
        turret_bounds.min.y,
        turret_bounds.max.y
    );

    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
    let hull_bounds = hull.mesh.bounds().expect("hull bounds");
    assert!(
        mounts.gun_trunnion.translation.y > hull_bounds.max.y + 0.05,
        "gun barrel y={:.2} must clear hull top y={:.2}",
        mounts.gun_trunnion.translation.y,
        hull_bounds.max.y
    );

    assert!(
        mounts.muzzle.translation.z > mounts.gun_trunnion.translation.z + 3.0,
        "muzzle should extend well beyond trunnion"
    );
}

#[test]
fn turret_sits_on_top_of_hull_not_embedded() {
    let vehicle = bake_vehicle(VehicleKind::T55A).expect("T-55A recipe");
    let turret = vehicle.submesh(SubmeshKind::Turret).expect("turret submesh");
    let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");

    let turret_bounds = turret.mesh.bounds().expect("turret bounds");
    let turret_body_min_y = turret
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| vertex.smoothing != SG_RING)
        .map(|vertex| vertex.position.y)
        .fold(f32::INFINITY, f32::min);

    // The hull surface *beneath the turret* (its xz footprint) — not the whole hull. Raised
    // structures elsewhere, like the engine-deck panels behind the turret, legitimately rise above
    // the deck; what must not happen is the turret sinking into the hull it sits on.
    let hull_top_under_turret = hull
        .mesh
        .vertices()
        .iter()
        .filter(|vertex| {
            vertex.position.x >= turret_bounds.min.x
                && vertex.position.x <= turret_bounds.max.x
                && vertex.position.z >= turret_bounds.min.z
                && vertex.position.z <= turret_bounds.max.z
        })
        .map(|vertex| vertex.position.y)
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        turret_body_min_y >= hull_top_under_turret - 0.02,
        "turret body bottom y={:.2} should sit at or above the hull under it y={:.2} (ring collar may overlap)",
        turret_body_min_y,
        hull_top_under_turret
    );
}

#[test]
fn vehicle_mesh_catalog_reports_new_gpu_mesh_uploads_once() {
    let mut catalog = VehicleMeshCatalog::default();
    let snapshot = TankSnapshot {
        tank_id: TankId(7),
        team: TeamId(1),
        vehicle: VehicleKind::T55A,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: VehicleKind::T55A.spec().gun.dispersion_mrad,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    };

    let objects = tank_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);
    let uploads = catalog.take_pending_meshes();

    assert_eq!(uploads.len(), 3);
    assert!(uploads.iter().all(|(_, mesh)| mesh.index_count() > 0));
    assert!(objects.iter().all(|object| uploads.iter().any(|(handle, _)| *handle == object.mesh)));
    assert!(catalog.take_pending_meshes().is_empty());

    let second_objects = tank_render_objects(&mut catalog, &snapshot, [0.30, 0.40, 0.28]);

    assert!(catalog.take_pending_meshes().is_empty());
    assert_eq!(objects, second_objects);
}

#[test]
fn distinct_hull_colors_share_one_mesh_and_tint_per_object() {
    let mut catalog = VehicleMeshCatalog::default();
    let snapshot = TankSnapshot {
        tank_id: TankId(7),
        team: TeamId(1),
        vehicle: VehicleKind::T55A,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: VehicleKind::T55A.spec().gun.dispersion_mrad,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    };

    let green = [0.30, 0.40, 0.28];
    let red = [0.46, 0.29, 0.25];

    let green_objects = tank_render_objects(&mut catalog, &snapshot, green);
    assert_eq!(catalog.take_pending_meshes().len(), 3, "first colour uploads each submesh once");
    assert!(catalog.take_pending_meshes().is_empty());

    let red_objects = tank_render_objects(&mut catalog, &snapshot, red);
    assert!(
        catalog.take_pending_meshes().is_empty(),
        "a second team colour must reuse the team-neutral meshes, not re-upload"
    );

    // The mesh is now identity-only; the team colour rides on the per-object tint.
    for (i, (green_obj, red_obj)) in green_objects.iter().zip(red_objects.iter()).enumerate() {
        assert_eq!(green_obj.mesh, red_obj.mesh, "submesh {i} should share one mesh handle");
    }
    assert!(green_objects.iter().all(|object| object.tint == green), "green tint per object");
    assert!(red_objects.iter().all(|object| object.tint == red), "red tint per object");
}

#[test]
fn destroyed_module_mask_darkens_the_matching_submesh_without_reuploading_meshes() {
    let mut catalog = VehicleMeshCatalog::default();
    let mut snapshot = TankSnapshot {
        tank_id: TankId(7),
        team: TeamId(1),
        vehicle: VehicleKind::T55A,
        position: [0.0, 0.0, 0.0],
        yaw_rad: 0.0,
        turret_yaw_rad: 0.0,
        turret_yaw_velocity_rad_s: 0.0,
        gun_pitch_rad: 0.0,
        hit_points: 900,
        reload_remaining_s: 0.0,
        aim_dispersion_mrad: VehicleKind::T55A.spec().gun.dispersion_mrad,
        module_hit_points: VehicleKind::T55A.spec().module_health.hit_points_by_slot(),
        destroyed_modules_mask: 0,
    };
    let base = [0.30, 0.40, 0.28];
    let healthy = tank_render_objects(&mut catalog, &snapshot, base);
    assert_eq!(catalog.take_pending_meshes().len(), 3);

    snapshot.destroyed_modules_mask =
        ModuleSlot::Gun.destroyed_mask_bit() | ModuleSlot::Turret.destroyed_mask_bit();
    let damaged = tank_render_objects(&mut catalog, &snapshot, base);

    assert!(catalog.take_pending_meshes().is_empty(), "damage tint must not rebuild GPU meshes");
    assert_eq!(healthy[0].mesh, damaged[0].mesh);
    assert_eq!(healthy[1].mesh, damaged[1].mesh);
    assert_eq!(healthy[2].mesh, damaged[2].mesh);
    assert_eq!(healthy[0].tint, damaged[0].tint, "healthy hull keeps team tint");
    assert_ne!(healthy[1].tint, damaged[1].tint, "destroyed turret darkens turret object");
    assert_ne!(healthy[2].tint, damaged[2].tint, "destroyed gun darkens gun object");
}
