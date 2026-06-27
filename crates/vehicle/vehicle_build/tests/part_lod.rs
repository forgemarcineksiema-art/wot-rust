//! Part-aware LOD: LOD selection happens on *named semantic parts* before any mesh is merged, so a
//! reduced tier is "the silhouette and mount-critical parts, detail dropped" — not a decimation of a
//! flattened LOD0 blob. These tests pin which keys survive each tier and that the runtime groups stay
//! intact.

use std::collections::BTreeSet;

use vehicle_build::{PartLod, t54_description};
use vehicle_geometry::{BakedVehicle, LodLevel, SubmeshKind, audit_reduction};

/// Total triangles across all of a bake's submeshes.
fn triangles(baked: &BakedVehicle) -> usize {
    baked.submeshes().iter().map(|s| s.mesh.triangle_count()).sum()
}

fn description() -> vehicle_build::VehicleDescription {
    t54_description()
}

/// The set of part-key names retained at `lod`.
fn retained_names(lod: LodLevel) -> BTreeSet<&'static str> {
    description().parts.iter().filter(|p| p.lod.kept_at(lod)).map(|p| p.key.name).collect()
}

#[test]
fn lod0_keeps_every_part() {
    let desc = description();
    assert!(desc.parts.iter().all(|p| p.lod.kept_at(LodLevel::Lod0)), "LOD0 carries everything");
}

#[test]
fn reduced_tiers_drop_exactly_the_detail_parts() {
    let desc = description();
    let detail_names: BTreeSet<&'static str> =
        desc.parts.iter().filter(|p| p.lod == PartLod::Detail).map(|p| p.key.name).collect();
    let kept_names: BTreeSet<&'static str> =
        desc.parts.iter().filter(|p| p.lod != PartLod::Detail).map(|p| p.key.name).collect();

    let lod1 = retained_names(LodLevel::Lod1);
    // Every dropped name is a Detail name that is not *also* carried by a non-detail instance.
    let purely_detail: BTreeSet<_> = detail_names.difference(&kept_names).copied().collect();
    for name in &purely_detail {
        assert!(!lod1.contains(name), "{name} is detail-only and must drop below LOD0");
    }
    for name in &kept_names {
        assert!(lod1.contains(name), "silhouette/mount-critical {name} must survive LOD1");
    }
    assert_eq!(retained_names(LodLevel::Lod2), lod1, "LOD1 and LOD2 keep the same part policy");
}

#[test]
fn no_mount_critical_part_ever_disappears() {
    let desc = description();
    for lod in [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2] {
        for part in desc.parts.iter().filter(|p| p.lod == PartLod::MountCritical) {
            assert!(part.lod.kept_at(lod), "mount-critical {} dropped at {lod:?}", part.key.name);
        }
    }
}

#[test]
fn the_turret_shell_and_barrel_are_mount_critical() {
    let desc = description();
    let critical: BTreeSet<&'static str> =
        desc.parts.iter().filter(|p| p.lod == PartLod::MountCritical).map(|p| p.key.name).collect();
    assert!(critical.contains("turret_shell"), "the turret shell anchors the traverse");
    assert!(critical.contains("gun_barrel"), "the barrel anchors elevation/muzzle");
}

#[test]
fn every_runtime_group_stays_non_empty_at_every_lod() {
    let desc = description();
    for lod in [LodLevel::Lod0, LodLevel::Lod1, LodLevel::Lod2] {
        for group in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
            let any = desc.parts.iter().any(|p| p.submesh == group && p.lod.kept_at(lod));
            assert!(any, "{group:?} must keep at least one part at {lod:?}");
        }
    }
}

#[test]
fn build_lod_merges_only_the_retained_parts_into_three_groups() {
    // The baked LOD1 vehicle must still carry all three runtime submeshes (hull/turret/gun).
    let baked = description().build_lod(LodLevel::Lod1);
    for group in [SubmeshKind::Hull, SubmeshKind::Turret, SubmeshKind::Gun] {
        let sub = baked.submeshes().iter().find(|s| s.kind == group);
        assert!(sub.is_some_and(|s| s.mesh.triangle_count() > 0), "{group:?} non-empty at LOD1");
    }
}

#[test]
fn part_aware_reduction_stays_inside_lod0_and_keeps_every_group() {
    let desc = description();
    let base = desc.build();
    for lod in [LodLevel::Lod1, LodLevel::Lod2] {
        let reduced = desc.build_reduced_lod(lod);
        audit_reduction(&base, &reduced)
            .unwrap_or_else(|e| panic!("part-aware {lod:?} failed audit: {e}"));
        assert_eq!(reduced.mounts(), base.mounts(), "{lod:?} must not move mount frames");
    }
}

#[test]
fn triangle_counts_fall_monotonically_down_the_lod_ladder() {
    // A benchmark, not a hard cap: each tier must be strictly lighter than the one above it. Absolute
    // budgets are deferred until the new topology is approved (per the R6 plan).
    let desc = description();
    let lod0 = triangles(&desc.build());
    let lod1 = triangles(&desc.build_reduced_lod(LodLevel::Lod1));
    let lod2 = triangles(&desc.build_reduced_lod(LodLevel::Lod2));
    assert!(lod1 < lod0, "LOD1 ({lod1}) must be lighter than LOD0 ({lod0})");
    assert!(lod2 < lod1, "LOD2 ({lod2}) must be lighter than LOD1 ({lod1})");
}

#[test]
fn lod0_part_aware_build_equals_the_full_bake() {
    // At LOD0 there is no reduction, so the part-aware path returns the full authored bake.
    let desc = description();
    assert_eq!(triangles(&desc.build_reduced_lod(LodLevel::Lod0)), triangles(&desc.build()));
}

#[test]
fn part_keys_are_unique_so_each_part_has_a_stable_identity() {
    let desc = description();
    let keys: BTreeSet<(&'static str, u16)> =
        desc.parts.iter().map(|p| (p.key.name, p.key.instance)).collect();
    assert_eq!(keys.len(), desc.parts.len(), "no two parts share a (name, instance) key");
}
