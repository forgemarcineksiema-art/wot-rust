//! Lineup-wide cost and rigging gates: deterministic + unique bake hashes, sane mount frames,
//! and triangle/vertex budgets. Shape/fit gates live in `all_vehicles.rs`.

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{
    BakedVehicle, GOLDEN_BAKE_HASHES, MeshBounds, SubmeshKind, VEHICLE_BUDGETS, bake_vehicle,
};

fn bake_all() -> Vec<BakedVehicle> {
    VehicleKind::ALL
        .into_iter()
        .map(|kind| bake_vehicle(kind).unwrap_or_else(|e| panic!("{kind:?} should bake: {e}")))
        .collect()
}

fn submesh_bounds(vehicle: &BakedVehicle, kind: SubmeshKind) -> MeshBounds {
    vehicle
        .submesh(kind)
        .unwrap_or_else(|| panic!("{:?} missing {kind:?} submesh", vehicle.kind()))
        .mesh
        .bounds()
        .unwrap_or_else(|| panic!("{:?} {kind:?} submesh has no bounds", vehicle.kind()))
}

#[test]
fn every_vehicle_bake_is_deterministic_and_hash_is_unique() {
    let first = bake_all();
    let second = bake_all();
    for (a, b) in first.iter().zip(&second) {
        assert_eq!(
            a.deterministic_hash(),
            b.deterministic_hash(),
            "{:?} bake is not deterministic",
            a.kind()
        );
    }
    for (i, a) in first.iter().enumerate() {
        for b in &first[i + 1..] {
            assert_ne!(
                a.deterministic_hash(),
                b.deterministic_hash(),
                "{:?} and {:?} collide on bake hash",
                a.kind(),
                b.kind()
            );
        }
    }
}

#[test]
fn every_vehicle_bake_hash_matches_golden_output() {
    // The golden table lives in `vehicle_geometry::budgets` so the Forge Studio report quotes
    // the same source this gate enforces. Re-record THERE on an intentional geometry change.
    let expected = GOLDEN_BAKE_HASHES;
    let actual: Vec<(VehicleKind, u64)> =
        bake_all().iter().map(|vehicle| (vehicle.kind(), vehicle.deterministic_hash())).collect();

    if actual.as_slice() != expected {
        let lines: String = actual
            .iter()
            .map(|(kind, hash)| format!("        (VehicleKind::{kind:?}, {hash}_u64),\n"))
            .collect();
        panic!(
            "bake hashes changed; if this geometry change is intentional, replace the golden \
             array with:\n{lines}"
        );
    }
}

#[test]
fn every_vehicle_has_sane_mount_frames() {
    for vehicle in bake_all() {
        let kind = vehicle.kind();
        let mounts = vehicle.mounts();
        let ring = mounts.turret_ring.translation;
        let trunnion = mounts.gun_trunnion.translation;
        let muzzle = mounts.muzzle.translation;
        let hitbox = HitboxProfile::for_vehicle(kind);

        assert!(ring.is_finite() && trunnion.is_finite() && muzzle.is_finite());

        // The turret ring sits on the hull deck, above ground and below the hitbox roof.
        let hull_top = submesh_bounds(&vehicle, SubmeshKind::Hull).max.y;
        let turret = submesh_bounds(&vehicle, SubmeshKind::Turret);
        assert!(ring.y > 0.0, "{kind:?} turret ring is underground");
        assert!(ring.y <= hitbox.center_y_m + hitbox.half_height_m, "{kind:?} ring above roof");

        // The trunnion sits ahead of the ring, inside the turret/casemate, and clears the hull.
        assert!(trunnion.z > ring.z, "{kind:?} trunnion not ahead of turret ring");
        assert!(
            trunnion.y >= turret.min.y && trunnion.y <= turret.max.y,
            "{kind:?} trunnion y {:.2} outside turret [{:.2}, {:.2}]",
            trunnion.y,
            turret.min.y,
            turret.max.y
        );
        assert!(trunnion.y > hull_top - 0.05, "{kind:?} gun would clip into the hull deck");

        // The muzzle extends well ahead of the trunnion and past the hitbox.
        assert!(muzzle.z > trunnion.z + 2.5, "{kind:?} barrel too stubby");
        assert!(muzzle.z > hitbox.half_length_m, "{kind:?} muzzle inside hitbox");
        assert!((muzzle.y - trunnion.y).abs() < 1.0e-4, "{kind:?} muzzle off the gun axis");
    }
}

#[test]
fn every_vehicle_respects_triangle_and_vertex_budgets() {
    let budgets = VEHICLE_BUDGETS;
    for vehicle in bake_all() {
        let kind = vehicle.kind();
        let tris = |sub| vehicle.submesh(sub).map_or(0, |s| s.mesh.triangle_count());
        let verts = |sub| vehicle.submesh(sub).map_or(0, |s| s.mesh.vertex_count());

        let hull = tris(SubmeshKind::Hull);
        let turret = tris(SubmeshKind::Turret);
        let gun = tris(SubmeshKind::Gun);
        let total = hull + turret + gun;
        let total_verts =
            verts(SubmeshKind::Hull) + verts(SubmeshKind::Turret) + verts(SubmeshKind::Gun);

        for (label, count, (min, max)) in [
            ("hull", hull, budgets.hull_tri),
            ("turret", turret, budgets.turret_tri),
            ("gun", gun, budgets.gun_tri),
            ("total", total, budgets.vehicle_tri),
        ] {
            assert!(
                (min..=max).contains(&count),
                "{kind:?} {label} {count} tris outside [{min}, {max}]"
            );
        }
        assert!(
            total_verts <= budgets.vehicle_vert_max,
            "{kind:?} total {total_verts} verts over budget {}",
            budgets.vehicle_vert_max
        );
    }
}
