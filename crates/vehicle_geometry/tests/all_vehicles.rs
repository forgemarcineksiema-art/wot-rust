//! Lineup-wide shape gates: every `VehicleKind` must bake into finite, hitbox-honest, and
//! visually distinct geometry. Cost and rigging gates (hashes, mounts, budgets) live in
//! `vehicle_budgets.rs`; per-vehicle detail for the T-55A slice lives in `vehicle_recipe.rs`.

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{BakedVehicle, MaterialRole, MeshBounds, SubmeshKind, bake_vehicle};

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
fn every_vehicle_bakes_finite_indexed_geometry() {
    for vehicle in bake_all() {
        let kind = vehicle.kind();
        for sub in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let submesh = vehicle.submesh(sub).expect("submesh present");
            let mesh = &submesh.mesh;
            assert!(mesh.vertex_count() > 0, "{kind:?} {sub:?} has no vertices");
            assert_eq!(mesh.indices().len() % 3, 0, "{kind:?} {sub:?} index count not triangles");
            let vcount = mesh.vertex_count() as u32;
            assert!(
                mesh.indices().iter().all(|&i| i < vcount),
                "{kind:?} {sub:?} has an out-of-range index"
            );
            assert!(
                mesh.vertices().iter().all(|v| v.position.is_finite()),
                "{kind:?} {sub:?} has a non-finite position"
            );
            assert!(
                mesh.vertices().iter().all(|v| v.normal.is_normalized()),
                "{kind:?} {sub:?} has a non-unit normal"
            );
        }
    }
}

/// The visible body (hull + turret/casemate) must sit inside the collision hitbox *and* fill it;
/// the gun barrel is excluded and must protrude past the hitbox like a real gun.
#[test]
fn every_vehicle_body_fits_and_fills_its_hitbox() {
    // Side/roof/end seating slack, and the deliberate underside sink that hides the belly.
    const EPS: f32 = 0.05;
    const SINK: f32 = 0.15;

    for vehicle in bake_all() {
        let kind = vehicle.kind();
        let hitbox = HitboxProfile::for_vehicle(kind);
        let body = vehicle.body_bounds().expect("body bounds");
        let gun = submesh_bounds(&vehicle, SubmeshKind::Gun);

        let top = hitbox.center_y_m + hitbox.half_height_m;
        let floor = hitbox.center_y_m - hitbox.half_height_m;

        // Containment.
        assert!(body.min.x >= -hitbox.half_width_m - EPS, "{kind:?} pokes left of hitbox");
        assert!(body.max.x <= hitbox.half_width_m + EPS, "{kind:?} pokes right of hitbox");
        assert!(body.min.z >= -hitbox.half_length_m - EPS, "{kind:?} pokes behind hitbox");
        assert!(body.max.z <= hitbox.half_length_m + EPS, "{kind:?} pokes ahead of hitbox");
        assert!(body.max.y <= top + EPS, "{kind:?} roof {:.2} pokes above {top:.2}", body.max.y);
        assert!(body.min.y >= floor - SINK, "{kind:?} belly {:.2} sinks past floor", body.min.y);

        // Fill — guards against narrow / stubby / flat regressions.
        assert!(
            body.max.x >= 0.88 * hitbox.half_width_m,
            "{kind:?} too narrow: {:.2} of {:.2}",
            body.max.x,
            hitbox.half_width_m
        );
        assert!(
            body.max.z >= 0.88 * hitbox.half_length_m,
            "{kind:?} too short: {:.2} of {:.2}",
            body.max.z,
            hitbox.half_length_m
        );
        assert!(body.max.y >= top - 0.30, "{kind:?} too flat: roof {:.2} vs {top:.2}", body.max.y);

        // The barrel must extend beyond the hitbox.
        assert!(
            gun.max.z > hitbox.half_length_m,
            "{kind:?} barrel {:.2} should protrude past hitbox length {:.2}",
            gun.max.z,
            hitbox.half_length_m
        );
    }
}

/// Each vehicle must read as a distinct shape — not just a recoloured copy. The signature mixes
/// body extents, turret/casemate extents, and gun length; any two vehicles must differ in at
/// least one of these by a readable margin.
#[test]
fn every_vehicle_has_segmented_tracks_on_both_sides() {
    for vehicle in bake_all() {
        let kind = vehicle.kind();
        let hull = vehicle.submesh(SubmeshKind::Hull).expect("hull submesh");
        let hitbox = HitboxProfile::for_vehicle(kind);
        let track_vertices = hull
            .mesh
            .vertices()
            .iter()
            .filter(|vertex| vertex.material == MaterialRole::TrackMetal)
            .collect::<Vec<_>>();

        assert!(track_vertices.len() >= 96, "{kind:?} needs segmented track shoes");

        let right_segments = distinct_axis_buckets(
            track_vertices.iter().filter_map(|vertex| {
                (vertex.position.x > hitbox.half_width_m * 0.82).then_some(vertex.position.z)
            }),
            0.12,
        );
        let left_segments = distinct_axis_buckets(
            track_vertices.iter().filter_map(|vertex| {
                (vertex.position.x < -hitbox.half_width_m * 0.82).then_some(vertex.position.z)
            }),
            0.12,
        );

        assert!(right_segments >= 8, "{kind:?} right track must read as separate shoes");
        assert!(left_segments >= 8, "{kind:?} left track must read as separate shoes");
    }
}

#[test]
fn each_vehicle_has_a_distinct_silhouette() {
    const MARGIN: f32 = 0.04;

    fn signature(vehicle: &BakedVehicle) -> [f32; 7] {
        let body = vehicle.body_bounds().expect("body bounds");
        let turret = submesh_bounds(vehicle, SubmeshKind::Turret);
        let trunnion = vehicle.mounts().gun_trunnion.translation;
        let muzzle = vehicle.mounts().muzzle.translation;
        [
            body.max.x - body.min.x,
            body.max.y - body.min.y,
            body.max.z - body.min.z,
            turret.max.x - turret.min.x,
            turret.max.y - turret.min.y,
            turret.max.z - turret.min.z,
            muzzle.z - trunnion.z,
        ]
    }

    let vehicles = bake_all();
    let signatures: Vec<[f32; 7]> = vehicles.iter().map(signature).collect();
    for (i, a) in signatures.iter().enumerate() {
        for (j, b) in signatures.iter().enumerate().skip(i + 1) {
            let distinct = a.iter().zip(b).any(|(x, y)| (x - y).abs() >= MARGIN);
            assert!(
                distinct,
                "{:?} and {:?} share an indistinct silhouette ({a:?} vs {b:?})",
                vehicles[i].kind(),
                vehicles[j].kind()
            );
        }
    }
}

fn distinct_axis_buckets(values: impl Iterator<Item = f32>, bucket_size: f32) -> usize {
    let mut buckets = values.map(|value| (value / bucket_size).round() as i32).collect::<Vec<_>>();
    buckets.sort_unstable();
    buckets.dedup();
    buckets.len()
}
