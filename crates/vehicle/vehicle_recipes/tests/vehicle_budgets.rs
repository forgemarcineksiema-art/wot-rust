//! Cost and rigging gates for the PROCEDURAL bake: deterministic + unique bake hashes, sane mount
//! frames, and triangle/vertex budgets. Shape/fit gates live in `all_vehicles.rs`.
//!
//! Not lineup-wide, despite baking `VehicleKind::ALL`: this measures what `bake_vehicle` produces,
//! and for the T-54 that is the legacy recipe, not the hybrid mesh the game actually draws. The
//! shipped meshes are gated by `vehicle_forge/tests/shipped_cost.rs`.

mod common;
use common::{bake_all, submesh_bounds};

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::SubmeshKind;
use vehicle_recipes::{VEHICLE_BUDGETS, bake_goldens, bake_vehicle};

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
    // The golden table is `vehicle_recipes/goldens/bake_hashes.txt` — the Forge Studio report
    // quotes the same source this gate enforces. Re-record ONE vehicle on an intentional
    // geometry change: `cargo run -p tools -- bless --vehicle <slug>`.
    let expected: Vec<(VehicleKind, u64)> =
        bake_goldens().iter().map(|row| (row.kind, row.recipe)).collect();
    let actual: Vec<(VehicleKind, u64)> =
        bake_all().iter().map(|vehicle| (vehicle.kind(), vehicle.deterministic_hash())).collect();
    let drifted: Vec<String> = actual
        .iter()
        .filter(|(kind, hash)| expected.iter().any(|(k, h)| k == kind && h != hash))
        .map(|(kind, hash)| format!("  {kind:?}: fresh {hash}"))
        .collect();
    let missing: Vec<String> = actual
        .iter()
        .filter(|(kind, _)| !expected.iter().any(|(k, _)| k == kind))
        .map(|(kind, _)| format!("  {kind:?} has no golden row"))
        .collect();
    assert!(
        drifted.is_empty() && missing.is_empty(),
        "bake hashes changed; if the geometry change is intentional, re-record each vehicle with \
         `cargo run -p tools -- bless --vehicle <slug>` and say the number in the commit:\n{}{}",
        drifted.join("\n"),
        missing.join("\n")
    );
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

/// The studio's live-override path: a modified blueprint bakes to a different shape, and the
/// override never leaks — the very next registered bake is byte-identical to the golden.
#[test]
fn blueprint_override_changes_the_bake_and_never_leaks() {
    let kind = VehicleKind::T34_85;
    let mut blueprint =
        game_core::VehicleBlueprint::for_vehicle(kind).expect("T-34-85 has a blueprint");
    blueprint.turret.roof_y -= 0.2;

    let overridden =
        vehicle_recipes::bake_vehicle_from_blueprint(&blueprint).expect("override bakes");
    let registered = bake_vehicle(kind).expect("registered bakes");
    assert_ne!(
        overridden.deterministic_hash(),
        registered.deterministic_hash(),
        "a lowered roof must produce a different bake"
    );
    assert_eq!(
        registered.deterministic_hash(),
        vehicle_recipes::golden_bake_hash(kind).expect("golden recorded"),
        "the override must not leak into the registered path"
    );
}
