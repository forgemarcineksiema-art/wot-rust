//! One test binary per crate.
//!
//! Every `tests/*.rs` file is its own program, linked against the whole dependency tree. On
//! 2026-09-06 the workspace had 314 of them: per heavy crate ~180 CPU-seconds of linking on
//! every gate, and `cargo clippy --all-targets` fronting 372 targets. Consolidated into one
//! `tests/suite/main.rs` per crate (each former file a `mod`), sim's test build after a one-file
//! touch went from 84 s to 6 s.
//!
//! The rule: a crate's `tests/` directory auto-discovers at most ONE target besides the goldens
//! named below — `suite/main.rs`, or a single file where a crate has only one. A second loose
//! file is a second link on every gate; it belongs in the suite as a module.
//!
//! The goldens stay separate on purpose: they re-record under an env var
//! (`gate_completeness.rs` names them), and a re-record must not share a process with a value
//! test reading the same file (`--test-threads=1` is their protocol).

use quality::workspace::{crate_manifests, repo_relative, workspace_root};

/// Test files that keep their own binary, with the reason.
const SEPARATE_BINARY_ALLOWLIST: &[(&str, &str)] = &[
    (
        "crates/apps/client/tests/look_goldens.rs",
        "renders through wgpu and re-records under WOT_UPDATE_GOLDENS; runs alone",
    ),
    (
        "crates/apps/tools/tests/studio_goldens.rs",
        "re-records the studio tiles under WOT_UPDATE_GOLDENS; runs alone",
    ),
    (
        "crates/runtime/net/tests/protocol_snapshots.rs",
        "re-records the wire fixtures under REGEN_WIRE_FIXTURES; runs alone",
    ),
];

#[test]
fn a_crate_links_one_test_binary_besides_its_goldens() {
    let root = workspace_root();
    let mut offenders = Vec::new();
    for manifest in crate_manifests(&root) {
        let Some(dir) = manifest.parent() else { continue };
        let tests = dir.join("tests");
        if !tests.is_dir() {
            continue;
        }
        let mut targets: Vec<String> = std::fs::read_dir(&tests)
            .expect("tests dir is readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                // cargo auto-discovers `tests/*.rs` and `tests/*/main.rs`.
                path.extension().is_some_and(|ext| ext == "rs") || path.join("main.rs").is_file()
            })
            .map(|path| repo_relative(&path, &root))
            .filter(|rel| !SEPARATE_BINARY_ALLOWLIST.iter().any(|(file, _)| file == rel))
            .collect();
        targets.sort();
        if targets.len() > 1 {
            offenders.push(format!(
                "{}: {} auto-discovered test targets — {}",
                repo_relative(dir, &root),
                targets.len(),
                targets.join(", ")
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "one test binary per crate: move the loose file into tests/suite/ and add a `mod` line to \
         tests/suite/main.rs (or name it in SEPARATE_BINARY_ALLOWLIST with the reason):\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_separate_binary_allowlist_names_only_files_that_still_exist() {
    let root = workspace_root();
    let missing: Vec<_> = SEPARATE_BINARY_ALLOWLIST
        .iter()
        .filter(|(file, _)| !root.join(file).is_file())
        .map(|(file, _)| *file)
        .collect();
    assert!(missing.is_empty(), "delete the entries, the files are gone: {missing:?}");
}
