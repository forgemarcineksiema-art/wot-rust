//! Architecture gate (W4 F0): a specific vehicle is DATA, and only data crates may name one.
//!
//! The fleet's most expensive asymmetry is not line count — it is that raising a vehicle to the
//! T-54's fidelity means writing Rust in half a dozen crates instead of editing data. Every
//! `VehicleKind::T54_1951` branch outside the data and content layers is one more place a new
//! vehicle has to be hand-threaded through, and one more place the fleet's technology is secretly
//! T-54-shaped.
//!
//! The rule: code may dispatch on a SPECIFIC vehicle variant only in `foundation/game_core` (the
//! data tables: catalogs, blueprints, layouts, slugs) and `crates/vehicle/` (the content
//! generators those tables point at). Everywhere else — runtime, render, apps, kernels — vehicles
//! are `VehicleKind` values flowing through generic machinery. The measured baseline is already
//! clean in the whole runtime layer; the allowlist below is the entire remaining debt, and the
//! W4 waves burn it.

use std::fs;

use quality::workspace::{
    crate_src_dirs, is_test_module_file, repo_relative, rust_files, workspace_root,
};

/// Files allowed to name specific vehicles, with the reason and the wave that burns the entry.
const DISPATCH_ALLOWLIST: &[(&str, &str)] = &[
    (
        "crates/apps/client/src/vehicle/display.rs",
        "Short display names (\"T-54\", \"Tiger II\") hardcoded in the CLIENT — UI data living in \
         the app layer. Burns in W4 F4: the short name becomes a `game_core` data accessor beside \
         the spec name.",
    ),
    // BURNED (W4 F3): `asset_catalog.rs`'s `if kind != T54_1951` became the data question the
    // render path already asked — `vehicle_has_cut_truth`, which reads whether the blueprint
    // carries the visual-detail block.
    (
        "crates/kernels/vehicle_geometry/src/recipes/",
        "Per-vehicle geometry recipes living in the KERNELS layer — content in L1, which the \
         target architecture puts in L2. A directory entry, deliberately: every file in it is \
         the same debt. Burns when the recipe registry moves into `crates/vehicle/` beside the \
         forge.",
    ),
    (
        "crates/kernels/vehicle_geometry/src/budgets.rs",
        "The per-vehicle triangle budgets and GOLDEN_BAKE_HASHES table — fleet data in the \
         kernels layer, the same debt and the same burn as the recipes directory.",
    ),
    (
        "crates/world/scene_build/src/review_views.rs",
        "The review scenes park specific vehicles for the look harness — presentation content \
         in the world layer. Burns when the lineup becomes data, which the visual programme \
         owns (blocked by the user's visual freeze).",
    ),
];

#[test]
fn specific_vehicles_are_named_only_in_data_and_content_crates() {
    let root = workspace_root();
    let variants = vehicle_variants(&root);
    let mut offenders = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        let relative_dir = repo_relative(&src_dir, &root);
        if relative_dir.starts_with("crates/foundation/game_core")
            || relative_dir.starts_with("crates/vehicle/")
            || relative_dir.starts_with("crates/tooling/")
        {
            continue;
        }
        for path in rust_files(&src_dir) {
            if is_test_module_file(&path) {
                continue;
            }
            let relative = repo_relative(&path, &root);
            if DISPATCH_ALLOWLIST.iter().any(|(entry, _)| relative.starts_with(entry)) {
                continue;
            }
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            // The convention (enforced by clippy's items_after_test_module) keeps inline test
            // modules last, so cutting at the marker strips them without a parser.
            let code = source.split("#[cfg(test)]").next().unwrap_or("");
            for (line_number, line) in code.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for variant in &variants {
                    if line.contains(&format!("VehicleKind::{variant}")) {
                        offenders.push(format!(
                            "{relative}:{}: names `VehicleKind::{variant}` — a specific vehicle \
                             outside the data/content layers is the fleet's technology being \
                             T-54-shaped (or {variant}-shaped) by hand",
                            line_number + 1,
                        ));
                    }
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "vehicles are data; only data and content crates may name one:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_allowlist_names_only_paths_that_still_exist() {
    let root = workspace_root();
    let stale: Vec<&str> = DISPATCH_ALLOWLIST
        .iter()
        .map(|(entry, _)| *entry)
        .filter(|entry| !root.join(entry.trim_end_matches('/')).exists())
        .collect();
    assert!(stale.is_empty(), "these allowlist entries outlived their paths: {stale:?}");
}

/// The enum's variant names, parsed from the declaration so a new vehicle is covered the day it
/// is appended — the same approach the identity-enum rules use. `ALL`/`PLAYABLE`/`SHIPPED` are
/// associated constants, not variants, and never match.
fn vehicle_variants(root: &std::path::Path) -> Vec<String> {
    let source = fs::read_to_string(root.join("crates/foundation/game_core/src/vehicle_kind.rs"))
        .expect("the vehicle enum should be readable");
    let start = source.find("pub enum VehicleKind").expect("the enum exists");
    let body_start = source[start..].find('{').expect("enum opens") + start + 1;
    let body_end = source[body_start..].find('}').expect("enum closes") + body_start;
    source[body_start..body_end]
        .lines()
        .filter_map(|line| {
            let name = line.trim().trim_end_matches(',');
            (!name.is_empty()
                && !name.starts_with("//")
                && !name.starts_with('#')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            .then(|| name.to_string())
        })
        .collect()
}
