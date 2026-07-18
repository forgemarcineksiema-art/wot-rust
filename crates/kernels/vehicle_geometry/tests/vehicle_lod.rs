//! Level-of-detail ladder gates: every vehicle must produce LOD0/LOD1/LOD2 that get strictly
//! lighter, stay deterministic, keep their mount frames, and never poke outside the LOD0 silhouette
//! (so the gameplay hitbox honesty proven for LOD0 carries to every reduced level).

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{BakedVehicle, LodLevel, SubmeshKind, bake_vehicle, bake_vehicle_lod};

const EPS: f32 = 1.0e-3;

// Absolute triangle budgets, tuned to the current lean procedural base. The plan's nominal
// 8k/4k/1k figures assumed a denser authoring pass; we keep the base cheap (many tanks on screen)
// and gate the real ladder instead of inflating triangle counts artificially.
// Ceiling follows VEHICLE_BUDGETS.vehicle_tri (raised for the wrap-arc band skin, 2026-07-18).
const LOD0_TRI_RANGE: std::ops::RangeInclusive<usize> = 250..=3950;
const LOD1_TRI_RANGE: std::ops::RangeInclusive<usize> = 200..=2000;
// LOD2 floor lowered 100 -> 80 with the legacy-fleet moving gear: wheels and shoe links left
// the German hull bakes for render-time instancing, so the slab-sided Jagdtiger legitimately
// reduces to ~88 baked triangles without its silhouette degrading.
const LOD2_TRI_RANGE: std::ops::RangeInclusive<usize> = 80..=900;

fn total_tris(vehicle: &BakedVehicle) -> usize {
    vehicle.submeshes().iter().map(|s| s.mesh.triangle_count()).sum()
}

#[test]
fn every_vehicle_lod_ladder_gets_strictly_lighter_within_budget() {
    for kind in VehicleKind::ALL {
        let lod0 = total_tris(&bake_vehicle_lod(kind, LodLevel::Lod0).unwrap());
        let lod1 = total_tris(&bake_vehicle_lod(kind, LodLevel::Lod1).unwrap());
        let lod2 = total_tris(&bake_vehicle_lod(kind, LodLevel::Lod2).unwrap());

        assert!(
            lod0 > lod1 && lod1 > lod2,
            "{kind:?} LOD ladder not strictly lighter: {lod0}/{lod1}/{lod2}"
        );
        assert!(
            lod1 as f32 <= 0.75 * lod0 as f32,
            "{kind:?} LOD1 {lod1} not meaningfully lighter than LOD0 {lod0}"
        );
        assert!(
            lod2 as f32 <= 0.45 * lod0 as f32,
            "{kind:?} LOD2 {lod2} not meaningfully lighter than LOD0 {lod0}"
        );

        assert!(LOD0_TRI_RANGE.contains(&lod0), "{kind:?} LOD0 {lod0} outside {LOD0_TRI_RANGE:?}");
        assert!(LOD1_TRI_RANGE.contains(&lod1), "{kind:?} LOD1 {lod1} outside {LOD1_TRI_RANGE:?}");
        assert!(LOD2_TRI_RANGE.contains(&lod2), "{kind:?} LOD2 {lod2} outside {LOD2_TRI_RANGE:?}");
    }
}

#[test]
fn lod0_equals_the_authored_bake() {
    for kind in VehicleKind::ALL {
        let authored = bake_vehicle(kind).unwrap();
        let lod0 = bake_vehicle_lod(kind, LodLevel::Lod0).unwrap();
        assert_eq!(
            authored.deterministic_hash(),
            lod0.deterministic_hash(),
            "{kind:?} LOD0 must be the untouched authored mesh"
        );
    }
}

#[test]
fn every_lod_is_deterministic() {
    for kind in VehicleKind::ALL {
        for level in [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2] {
            let a = bake_vehicle_lod(kind, level).unwrap();
            let b = bake_vehicle_lod(kind, level).unwrap();
            assert_eq!(
                a.deterministic_hash(),
                b.deterministic_hash(),
                "{kind:?} {} bake is not deterministic",
                level.slug()
            );
        }
    }
}

#[test]
fn reduced_lods_preserve_mount_frames() {
    for kind in VehicleKind::ALL {
        let lod0 = bake_vehicle_lod(kind, LodLevel::Lod0).unwrap();
        for level in [LodLevel::Lod1, LodLevel::Lod2] {
            let reduced = bake_vehicle_lod(kind, level).unwrap();
            assert_eq!(
                reduced.mounts(),
                lod0.mounts(),
                "{kind:?} {} dropped its mount frames",
                level.slug()
            );
        }
    }
}

#[test]
fn reduced_lods_stay_inside_lod0_silhouette_and_hitbox() {
    for kind in VehicleKind::ALL {
        let hitbox = HitboxProfile::for_vehicle(kind);
        let lod0_body = bake_vehicle_lod(kind, LodLevel::Lod0).unwrap().body_bounds().unwrap();
        for level in [LodLevel::Lod1, LodLevel::Lod2] {
            let reduced = bake_vehicle_lod(kind, level).unwrap();
            // Body (hull + turret) must not exceed the LOD0 silhouette (clustering pulls inward).
            let body = reduced.body_bounds().unwrap();
            assert!(body.min.x >= lod0_body.min.x - EPS, "{kind:?} {} grows left", level.slug());
            assert!(body.max.x <= lod0_body.max.x + EPS, "{kind:?} {} grows right", level.slug());
            assert!(body.min.y >= lod0_body.min.y - EPS, "{kind:?} {} grows down", level.slug());
            assert!(body.max.y <= lod0_body.max.y + EPS, "{kind:?} {} grows up", level.slug());
            assert!(body.min.z >= lod0_body.min.z - EPS, "{kind:?} {} grows back", level.slug());
            assert!(body.max.z <= lod0_body.max.z + EPS, "{kind:?} {} grows fwd", level.slug());

            // And it still respects the gameplay hitbox in x/z.
            assert!(
                body.min.x >= -hitbox.half_width_m - EPS,
                "{kind:?} {} pokes left of hitbox",
                level.slug()
            );
            assert!(
                body.max.x <= hitbox.half_width_m + EPS,
                "{kind:?} {} pokes right of hitbox",
                level.slug()
            );
            assert!(
                body.min.z >= -hitbox.half_length_m - EPS,
                "{kind:?} {} pokes behind hitbox",
                level.slug()
            );
            assert!(
                body.max.z <= hitbox.half_length_m + EPS,
                "{kind:?} {} pokes ahead of hitbox",
                level.slug()
            );

            // The three named submeshes survive reduction (a silhouette is still a whole tank).
            for sub in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
                let mesh = &reduced.submesh(sub).expect("submesh survives LOD").mesh;
                assert!(mesh.triangle_count() > 0, "{kind:?} {} lost its {sub:?}", level.slug());
            }
        }
    }
}
