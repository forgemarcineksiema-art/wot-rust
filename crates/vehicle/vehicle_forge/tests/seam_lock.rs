//! The mesh-source seam after Forge 2.0 K1: one rule, every vehicle a description, and not a
//! byte moved.
//!
//! Before K1 `mesh_source.rs`, `production_bake.rs`, `cost.rs`, the studio and the compiler each
//! forked on `VehicleKind::T54_1951`. Now the description declares its fidelity and its LOD
//! strategy, and the forge reads them. This file pins two things: that the seam's files no longer
//! name a vehicle, and that the bake every consumer receives is the bake it received before —
//! the T-54's hash from `vehicle_build/tests/part_library.rs`, the seven sketches' from the
//! recipe goldens, and the reduced tiers against the reduction each path used to run.

use std::fs;
use std::path::Path;

use game_core::VehicleKind;
use vehicle_build::{Fidelity, t54_description};
use vehicle_forge::{
    BakeProfile, authoritative_baked_vehicle, bake_production_vehicle, shipped_fidelity,
};
use vehicle_geometry::{LodLevel, reduce_vehicle};
use vehicle_recipes::{bake_vehicle, golden_bake_hash};

const T54_LOD0_HASH: u64 = 9_296_666_834_409_964_133;

/// A MIXED sketch (Forge 2.0 K3): its recipe pieces still stand and library parts ride on them,
/// so its bake is neither the recipe golden nor a benchmark build. Pinned here; a construction
/// PR that changes the mix re-records the row with the number in its message.
const MIXED_LOD0_HASHES: &[(VehicleKind, u64)] = &[
    // The Tiger I with the STT-sheet fittings over its five recipe pieces (K3-2b).
    (VehicleKind::TigerI, 7_009_963_337_679_654_198),
];

fn mixed_hash(kind: VehicleKind) -> Option<u64> {
    MIXED_LOD0_HASHES.iter().find(|(k, _)| *k == kind).map(|(_, h)| *h)
}

#[test]
fn every_vehicle_bakes_to_the_hash_it_baked_to_before_the_seam_was_one_rule() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        let baked = authoritative_baked_vehicle(kind).expect("bakes");
        let expected = match (shipped_fidelity(kind), mixed_hash(kind)) {
            (Fidelity::Benchmark, _) => T54_LOD0_HASH,
            (Fidelity::Sketch, Some(mixed)) => mixed,
            (Fidelity::Sketch, None) => golden_bake_hash(kind).expect("a recipe golden"),
        };
        assert_eq!(
            baked.deterministic_hash(),
            expected,
            "{kind:?}: the seam changed the bake — wrapping a recipe as a description must be \
             byte-exact"
        );
        checked += 1;
    }
    assert_eq!(checked, VehicleKind::PLAYABLE.len());
}

#[test]
fn the_reduced_tiers_are_the_reductions_each_path_ran_before() {
    let mut checked = 0;
    for kind in VehicleKind::PLAYABLE {
        for (profile, level) in
            [(BakeProfile::Lod1, LodLevel::Lod1), (BakeProfile::Lod2, LodLevel::Lod2)]
        {
            let production = bake_production_vehicle(kind, profile).expect("bakes");
            let before = match (shipped_fidelity(kind), mixed_hash(kind)) {
                (Fidelity::Benchmark, _) => t54_description().build_reduced_lod(level),
                // A mixed sketch reduces its own composed bake whole-mesh, as the recipe did.
                (Fidelity::Sketch, Some(_)) => {
                    reduce_vehicle(&authoritative_baked_vehicle(kind).expect("bakes"), level)
                }
                (Fidelity::Sketch, None) => {
                    reduce_vehicle(&bake_vehicle(kind).expect("recipe"), level)
                }
            };
            assert_eq!(
                production.deterministic_hash(),
                before.deterministic_hash(),
                "{kind:?} {level:?}: the description's LOD strategy must reproduce the old path"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, VehicleKind::PLAYABLE.len() * 2);
}

#[test]
fn exactly_one_vehicle_is_at_the_benchmark_fidelity_today() {
    let benchmarks: Vec<VehicleKind> = VehicleKind::PLAYABLE
        .into_iter()
        .filter(|kind| shipped_fidelity(*kind) == Fidelity::Benchmark)
        .collect();
    assert_eq!(
        benchmarks,
        vec![VehicleKind::T54_1951],
        "K3 moves this list, one vehicle at a time"
    );
}

/// The seam's files hold no vehicle's name: a vehicle migrates by data (its blueprint gains a
/// complete visual), never by a new match arm in the forge.
#[test]
fn the_seam_names_no_vehicle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in [
        "src/mesh_source.rs",
        "src/production_bake.rs",
        "src/cost.rs",
        "src/compiler.rs",
        "src/artifact/studio.rs",
    ] {
        let source = fs::read_to_string(root.join(file)).expect(file);
        // The production half only: a file's own unit tests may name the vehicle they probe.
        let production = source.split("#[cfg(test)]").next().unwrap_or("");
        let offenders: Vec<&str> = production
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .filter(|line| {
                line.contains("VehicleKind::") && !line.contains("VehicleKind::PLAYABLE")
            })
            .collect();
        assert!(
            offenders.is_empty(),
            "{file} dispatches on a specific vehicle again:\n  {}",
            offenders.join("\n  ")
        );
    }
}
