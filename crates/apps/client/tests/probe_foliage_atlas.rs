//! Instrument gate (Inny Poziom F1): every probe that draws a battlefield binds the leaf
//! atlas.
//!
//! The renderer's default foliage texture is a 1×1 opaque white no-op, so a probe that
//! forgets `set_foliage_atlas` draws every leaf card and every impostor as a solid white
//! rectangle — and its frames and its frame times are about a world that does not exist.
//! `perf_capture` and the map-view probes did exactly that from Drzewa 3.0 PR6 (the first
//! geometry with real UVs) to F1, and the review path had lost the bind once before
//! (`look_harness.rs`). Three times is a rule: a battlefield probe binds through
//! `bind_battle_foliage_atlas` (or the harness, which binds), or this test refuses it.

use std::fs;
use std::path::Path;

/// A probe "draws a battlefield" when it builds an offscreen scene renderer AND compiles a
/// shipped map. Garage and studio probes build renderers over hangars and turntables and
/// carry no foliage; they are outside the rule.
fn draws_a_battlefield(source: &str) -> bool {
    source.contains("for_offscreen") && source.contains("battlefield(")
}

fn binds_the_atlas(source: &str) -> bool {
    source.contains("bind_battle_foliage_atlas(") || source.contains("set_foliage_atlas(")
}

#[test]
fn every_battlefield_probe_binds_the_leaf_atlas() {
    let probes = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples").join("probe");
    let mut checked = 0;
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&probes).expect("the probe directory exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("probe source reads");
        if !draws_a_battlefield(&source) {
            continue;
        }
        checked += 1;
        if !binds_the_atlas(&source) {
            offenders.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(checked >= 10, "the rule covers the battlefield probes, found only {checked}");
    assert!(
        offenders.is_empty(),
        "battlefield probes drawing white trees (no leaf-atlas bind): {offenders:?}"
    );
}

/// The helper itself stays in the probe binary, named as the doctrine names it.
#[test]
fn the_probe_binary_carries_the_shared_bind() {
    let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/probe/main.rs");
    let source = fs::read_to_string(main).expect("probe main reads");
    assert!(
        source.contains("pub(crate) fn bind_battle_foliage_atlas("),
        "the shared bind left the probe binary"
    );
}
