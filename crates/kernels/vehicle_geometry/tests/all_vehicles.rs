//! Lineup-wide shape gates: every `VehicleKind` must bake into finite, hitbox-honest, and
//! visually distinct geometry. Cost and rigging gates (hashes, mounts, budgets) live in
//! `vehicle_budgets.rs`; legacy T-55A compatibility detail lives in `vehicle_recipe.rs`.

use game_core::{HitboxProfile, VehicleKind};
use vehicle_geometry::{
    BakedVehicle, GearPart, MaterialRole, MeshBounds, RunningGearKinematics, SubmeshKind,
    bake_vehicle, running_gear_placements,
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

/// The gameplay turret volume (`HitboxProfile::with_turret_plan`) must be a tight box around the
/// visual turret: contained (a shot into the box meets metal) and filled (the box is not padded
/// with empty air a shot could "hit"). The ring pivot the volume traverses about must sit inside
/// it. This is what keeps the turret hit model honest against the baked geometry.
#[test]
fn every_vehicle_turret_fits_and_fills_its_turret_plan() {
    const EPS: f32 = 0.05;

    for vehicle in bake_all() {
        let kind = vehicle.kind();
        let hitbox = HitboxProfile::for_vehicle(kind);
        let turret = submesh_bounds(&vehicle, SubmeshKind::Turret);
        let z_lo = hitbox.turret_center_z_m - hitbox.turret_half_length_m;
        let z_hi = hitbox.turret_center_z_m + hitbox.turret_half_length_m;

        // Containment: the visual turret may not poke out of its gameplay volume.
        assert!(
            turret.min.x >= -hitbox.turret_half_width_m - EPS,
            "{kind:?} turret pokes left of its plan: {:.2} vs {:.2}",
            turret.min.x,
            -hitbox.turret_half_width_m
        );
        assert!(
            turret.max.x <= hitbox.turret_half_width_m + EPS,
            "{kind:?} turret pokes right of its plan: {:.2} vs {:.2}",
            turret.max.x,
            hitbox.turret_half_width_m
        );
        assert!(
            turret.min.z >= z_lo - EPS,
            "{kind:?} turret pokes behind its plan: {:.2} vs {z_lo:.2}",
            turret.min.z
        );
        assert!(
            turret.max.z <= z_hi + EPS,
            "{kind:?} turret pokes ahead of its plan: {:.2} vs {z_hi:.2}",
            turret.max.z
        );

        // Fill: the volume may not be padded much wider than the visible turret.
        assert!(
            turret.max.x >= 0.85 * hitbox.turret_half_width_m,
            "{kind:?} turret plan too wide: turret reaches {:.2} of {:.2}",
            turret.max.x,
            hitbox.turret_half_width_m
        );
        assert!(
            (turret.max.z - turret.min.z) >= 0.85 * 2.0 * hitbox.turret_half_length_m,
            "{kind:?} turret plan too long: turret spans {:.2} of {:.2}",
            turret.max.z - turret.min.z,
            2.0 * hitbox.turret_half_length_m
        );

        // The plan stays inside the hull plan, and the traverse pivot sits inside the plan.
        assert!(hitbox.turret_half_width_m <= hitbox.half_width_m);
        assert!(z_hi <= hitbox.half_length_m && z_lo >= -hitbox.half_length_m);
        let ring_z = vehicle.mounts().turret_ring.translation.z;
        assert!(
            ring_z > z_lo && ring_z < z_hi,
            "{kind:?} ring pivot z {ring_z:.2} outside turret plan [{z_lo:.2}, {z_hi:.2}]"
        );
    }
}

#[test]
fn every_vehicle_has_segmented_tracks_on_both_sides() {
    for vehicle in bake_all() {
        let kind = vehicle.kind();
        // Blueprint vehicles animate their belt: the shoe links are instanced from the kinematics,
        // not baked into the hull. Verify the loop is segmented (many links) on both sides instead.
        if let Some(kin) = RunningGearKinematics::for_vehicle(kind) {
            let placements = running_gear_placements(&kin, 0.0, 0.0);
            let links = placements.iter().filter(|p| p.part == GearPart::Link).count();
            assert!(
                links >= 16,
                "{kind:?} animated belt must be segmented into many shoe links ({links})"
            );
            continue;
        }
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

/// Each vehicle must read as a distinct shape — not just a recoloured copy. The signature mixes
/// body extents, turret/casemate extents, and gun length; any two vehicles must differ in at
/// least one of these by a readable margin.
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
