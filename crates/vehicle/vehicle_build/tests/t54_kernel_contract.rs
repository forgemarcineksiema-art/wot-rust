//! The T-54 as the executable kernel acceptance fixture. This is the reference every later kernel
//! is measured against: at every LOD the submeshes pass the shared mesh-quality contract, the body
//! stays inside its gameplay volumes, the mount frames never move, the LOD ladder falls
//! monotonically and stays deterministic, and the cast shell keeps its T-54 silhouette properties.

use game_core::{HitboxProfile, MountFrames, TurretLoftVisual, VehicleBlueprint, VehicleKind};
use vehicle_build::{t54_description, t54_turret_loft};
use vehicle_geometry::{BakedVehicle, LodLevel, OPEN_OR_CLOSED_MESH, SubmeshKind, reduce_vehicle};

const LODS: [LodLevel; 3] = [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2];

/// The production LOD bake: filter parts by their LOD policy, then cluster-reduce — the same two
/// steps the Forge runs.
fn baked_at(level: LodLevel) -> BakedVehicle {
    reduce_vehicle(&t54_description().build_lod(level), level)
}

fn turret_visual() -> TurretLoftVisual {
    VehicleBlueprint::for_vehicle(VehicleKind::T54_1951).unwrap().hybrid().unwrap().turret_loft
}

fn total_triangles(baked: &BakedVehicle) -> usize {
    baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum()
}

// ---- LOD-wide contract -------------------------------------------------------------------------

#[test]
fn lod0_submeshes_validate_against_the_shared_contract() {
    let baked = baked_at(LodLevel::Lod0);
    for submesh in baked.submeshes() {
        submesh
            .mesh
            .validate_quality(OPEN_OR_CLOSED_MESH)
            .unwrap_or_else(|e| panic!("{:?} authored submesh is unclean: {e}", submesh.kind));
    }
}

#[test]
fn reduced_lods_carry_no_catastrophic_mesh_defects() {
    // Vertex-cluster decimation can introduce non-manifold edges and zeroed cluster normals — both
    // acceptable for a render LOD — but it must never emit invalid indices, non-finite attributes,
    // or degenerate triangles, which would break the bake or the rasteriser.
    for level in [LodLevel::Lod1, LodLevel::Lod2] {
        for submesh in baked_at(level).submeshes() {
            let report = submesh.mesh.quality_report(OPEN_OR_CLOSED_MESH);
            assert_eq!(report.invalid_indices, 0, "{level:?} {:?} invalid indices", submesh.kind);
            assert_eq!(report.non_finite_vertices, 0, "{level:?} {:?} non-finite", submesh.kind);
            assert_eq!(
                report.degenerate_triangles, 0,
                "{level:?} {:?} degenerate triangles",
                submesh.kind
            );
        }
    }
}

#[test]
fn body_stays_inside_its_gameplay_volumes_at_every_lod() {
    const EPS: f32 = 0.05;
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    for level in LODS {
        let baked = baked_at(level);
        let body = baked.body_bounds().expect("body bounds");
        assert!(body.min.x >= -hitbox.half_width_m - EPS, "{level:?} body left");
        assert!(body.max.x <= hitbox.half_width_m + EPS, "{level:?} body right");
        assert!(body.min.z >= -hitbox.half_length_m - EPS, "{level:?} body rear");
        assert!(body.max.z <= hitbox.half_length_m + EPS, "{level:?} body front");
        assert!(
            body.max.y <= hitbox.center_y_m + hitbox.half_height_m + EPS,
            "{level:?} body roof"
        );
    }
}

#[test]
fn the_gun_extends_beyond_the_hitbox_by_design() {
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let gun = baked_at(LodLevel::Lod0)
        .submesh(SubmeshKind::Gun)
        .expect("gun submesh")
        .mesh
        .bounds()
        .expect("gun bounds");
    assert!(gun.max.z > hitbox.half_length_m, "barrel must protrude past the hull");
}

#[test]
fn mount_frames_are_unchanged_across_every_lod() {
    let blueprint_mounts = MountFrames::for_vehicle(VehicleKind::T54_1951);
    for level in LODS {
        assert_eq!(*baked_at(level).mounts(), blueprint_mounts, "{level:?} mounts moved");
    }
}

#[test]
fn triangle_counts_fall_monotonically_through_the_lod_ladder() {
    let counts: Vec<usize> = LODS.iter().map(|&l| total_triangles(&baked_at(l))).collect();
    assert!(counts[0] > counts[1], "LOD0 {} must exceed LOD1 {}", counts[0], counts[1]);
    assert!(counts[1] > counts[2], "LOD1 {} must exceed LOD2 {}", counts[1], counts[2]);
}

#[test]
fn lower_lods_preserve_all_mount_critical_submeshes() {
    // The hull, turret and gun all carry mount-critical or silhouette parts, so none may vanish at
    // the reduced tiers — the pose chain and the readable silhouette must survive.
    for level in LODS {
        let baked = baked_at(level);
        for kind in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let submesh = baked.submesh(kind).unwrap_or_else(|| panic!("{level:?} lost {kind:?}"));
            assert!(submesh.mesh.triangle_count() > 0, "{level:?} {kind:?} collapsed to nothing");
        }
    }
}

#[test]
fn every_lod_is_deterministic() {
    for level in LODS {
        assert_eq!(
            baked_at(level).deterministic_hash(),
            baked_at(level).deterministic_hash(),
            "{level:?} bake is not deterministic"
        );
    }
}

// ---- Cast-shell silhouette properties ----------------------------------------------------------

#[test]
fn the_cast_shell_is_lower_and_wider_than_tall() {
    let b = t54_turret_loft(&turret_visual()).bounds().expect("shell bounds");
    let (w, d, h) = (b.max.x - b.min.x, b.max.z - b.min.z, b.max.y - b.min.y);
    assert!(w > h, "shell must be wider ({w:.2}) than tall ({h:.2})");
    assert!(d > h, "shell must be deeper ({d:.2}) than tall ({h:.2})");
}

#[test]
fn the_low_station_is_wider_than_the_roof_station() {
    let v = turret_visual();
    let low = v.stations[1].half_width; // widest, just above the ring seat
    let roof = v.stations.last().unwrap().half_width;
    assert!(low > roof + 0.2, "low station {low:.2} must overhang the roof {roof:.2}");
}

#[test]
fn the_front_reach_exceeds_the_rear_reach_at_a_matching_height() {
    // Front-heaviness is a per-station property (front half-length >= rear at the same y); the
    // absolute Z extent is deliberately offset by the rear-pulled bustle (negative z_center) and
    // the front embrasure recess, so it is not the right measure.
    let v = turret_visual();
    let mut strictly_front_heavy = false;
    for s in &v.stations {
        assert!(
            s.half_len_front >= s.half_len_rear,
            "station at y={:.2} is not front-heavy: front {:.2} rear {:.2}",
            s.y,
            s.half_len_front,
            s.half_len_rear
        );
        strictly_front_heavy |= s.half_len_front > s.half_len_rear + 1.0e-4;
    }
    assert!(strictly_front_heavy, "at least one station must reach further front than rear");
}

#[test]
fn the_cheeks_are_symmetric_about_the_longitudinal_plane() {
    let mesh = t54_turret_loft(&turret_visual());
    let b = mesh.bounds().expect("bounds");
    assert!((b.max.x + b.min.x).abs() < 1.0e-3, "shell bounds are symmetric in X");

    // The cheek bulge itself is mirrored: the widest +X vertex matches the widest -X vertex.
    let max_x = mesh.vertices().iter().map(|v| v.position.x).fold(f32::MIN, f32::max);
    let min_x = mesh.vertices().iter().map(|v| v.position.x).fold(f32::MAX, f32::min);
    assert!((max_x + min_x).abs() < 1.0e-3, "the cheeks bulge symmetrically: {max_x} vs {min_x}");
}

#[test]
fn the_front_embrasure_recess_stays_within_the_turret_plan() {
    let hitbox = HitboxProfile::for_vehicle(VehicleKind::T54_1951);
    let b = t54_turret_loft(&turret_visual()).bounds().expect("bounds");
    let z_hi = hitbox.turret_center_z_m + hitbox.turret_half_length_m;
    assert!(b.max.z <= z_hi + 0.05, "the embrasure/front must not escape the plan");
}

#[test]
fn the_cupola_is_a_separate_part_above_the_cast_roof() {
    let v = turret_visual();
    let roof_y = v.stations.last().unwrap().y;
    // The lofted shell itself tops out at the roof station — the cupola is NOT part of it.
    let shell_top = t54_turret_loft(&v).bounds().expect("bounds").max.y;
    assert!(shell_top <= roof_y + 0.02, "the cast shell tops at its roof station, not a cupola");
    // But the assembled turret submesh carries the separate cupola drum proud of that roof.
    let turret = t54_description()
        .build()
        .submesh(SubmeshKind::Turret)
        .expect("turret")
        .mesh
        .bounds()
        .expect("bounds");
    assert!(turret.max.y > roof_y + 0.05, "the cupola rides above the cast roof");
}

/// The aperture is closed at neutral elevation. This used to compare the gun submesh's bounding
/// box against the shell's, which the BARREL satisfies on its own from four metres away — the
/// assertion could not fail whatever the mount looked like. It measures the thing that closes the
/// hole now, which is the canvas.
#[test]
fn the_cover_closes_the_embrasure_without_a_front_gap() {
    let v = turret_visual();
    let baked = t54_description().build();
    let gun = &baked.submesh(SubmeshKind::Gun).expect("gun").mesh;
    let shell = t54_turret_loft(&v).bounds().expect("shell bounds");
    let fabric = gun
        .vertices()
        .iter()
        .filter(|vertex| vertex.material == vehicle_geometry::MaterialRole::Canvas)
        .map(|vertex| vertex.position.z)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        fabric >= shell.max.z - 0.02,
        "the cover front {fabric:.2} must reach the shell front {:.2}",
        shell.max.z
    );
}
