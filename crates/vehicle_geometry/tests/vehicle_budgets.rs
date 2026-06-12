//! Lineup-wide cost and rigging gates: deterministic + unique bake hashes, sane mount frames,
//! and triangle/vertex budgets. Shape/fit gates live in `all_vehicles.rs`.

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{BakedVehicle, MeshBounds, SubmeshKind, bake_vehicle};

// Triangle budgets (per submesh and per vehicle). Upper bounds keep runtime cost stable as the
// tank count grows; lower bounds guard against silhouettes regressing back into plain boxes.
const HULL_TRI_MAX: usize = 1200;
const TURRET_TRI_MAX: usize = 900;
const GUN_TRI_MAX: usize = 500;
const VEHICLE_TRI_MAX: usize = 2200;
const VEHICLE_VERT_MAX: usize = 6000;

const HULL_TRI_MIN: usize = 120;
const TURRET_TRI_MIN: usize = 24;
const GUN_TRI_MIN: usize = 24;
const VEHICLE_TRI_MIN: usize = 250;

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
    let expected = [
        (VehicleKind::PrototypeMedium, 7_613_624_468_440_756_419_u64),
        (VehicleKind::T54_1951, 9_323_884_532_056_781_199_u64),
        (VehicleKind::T55A, 16_216_939_546_650_613_941_u64),
        (VehicleKind::TigerI, 16_333_030_985_719_378_589_u64),
        (VehicleKind::TigerII, 1_086_861_276_510_338_113_u64),
        (VehicleKind::Jagdtiger, 4_893_810_432_222_783_231_u64),
        (VehicleKind::PantherII, 10_271_004_082_742_695_148_u64),
    ];
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

        assert!(
            (HULL_TRI_MIN..=HULL_TRI_MAX).contains(&hull),
            "{kind:?} hull {hull} tris outside [{HULL_TRI_MIN}, {HULL_TRI_MAX}]"
        );
        assert!(
            (TURRET_TRI_MIN..=TURRET_TRI_MAX).contains(&turret),
            "{kind:?} turret {turret} tris outside [{TURRET_TRI_MIN}, {TURRET_TRI_MAX}]"
        );
        assert!(
            (GUN_TRI_MIN..=GUN_TRI_MAX).contains(&gun),
            "{kind:?} gun {gun} tris outside [{GUN_TRI_MIN}, {GUN_TRI_MAX}]"
        );
        assert!(
            (VEHICLE_TRI_MIN..=VEHICLE_TRI_MAX).contains(&total),
            "{kind:?} total {total} tris outside [{VEHICLE_TRI_MIN}, {VEHICLE_TRI_MAX}]"
        );
        assert!(
            total_verts <= VEHICLE_VERT_MAX,
            "{kind:?} total {total_verts} verts over budget {VEHICLE_VERT_MAX}"
        );
    }
}
