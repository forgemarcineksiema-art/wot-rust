//! Architecture gate: module layout says the same thing everywhere it says anything.
//!
//! Two rules survived measurement. The rest of the naming programme did not, and the numbers are
//! recorded at the bottom of this file, because "we decided not to" is worth more than a rule
//! nobody can satisfy.
//!
//! Naming is not decoration here for one concrete reason: `duplication.rs` decides whether to scan
//! a file by looking at its NAME. `tests.rs` and `*_tests.rs` are skipped. So the convention a
//! file happens to follow changes which gates see it — which is exactly why the conventions
//! themselves have to be legible rather than folkloric.

use std::collections::BTreeMap;
use std::path::Path;

use quality::workspace::{crate_src_dirs, repo_relative, rust_files, workspace_root};

/// Crates allowed to use both module styles at once, with the count at landing.
///
/// Neither style is wrong — `foo/mod.rs` and `foo.rs` beside `foo/` both compile and both read
/// fine. Using BOTH inside one crate is the cost: a reader looking for a module has to know which
/// convention that particular module happened to be born under, and every new module re-opens the
/// question. Five crates mix today; four do not.
const MIXED_MODULE_STYLE_ALLOWLIST: &[(&str, &str)] = &[
    ("crates/apps/client", "5 mod.rs against 5 sibling files"),
    ("crates/foundation/game_core", "2 mod.rs against 3 sibling files"),
    ("crates/kernels/vehicle_geometry", "2 mod.rs against 2 sibling files"),
    ("crates/runtime/sim", "1 mod.rs against 1 sibling file"),
    ("crates/vehicle/vehicle_forge", "1 mod.rs against 2 sibling files"),
];

#[test]
fn no_module_repeats_the_name_of_the_directory_it_is_in() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for src_dir in crate_src_dirs(&root) {
        for path in rust_files(&src_dir) {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { continue };
            let Some(parent) = path.parent().and_then(|dir| dir.file_name()?.to_str()) else {
                continue;
            };
            if parent == "src" || stem == "mod" {
                continue;
            }
            if let Some(tail) = stem.strip_prefix(&format!("{parent}_")) {
                offenders.push(format!(
                    "{} — the directory already says `{parent}`, so this reads as \
                     `{parent}::{parent}_{tail}`; name it `{tail}.rs`",
                    repo_relative(&path, &root),
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a module that repeats its directory is a path said twice:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn a_crate_picks_one_module_style_and_keeps_it() {
    let root = workspace_root();
    let mut styles: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for src_dir in crate_src_dirs(&root) {
        let crate_dir = src_dir.parent().unwrap_or(&src_dir);
        let key = repo_relative(crate_dir, &root);
        for path in rust_files(&src_dir) {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else { continue };
            let entry = styles.entry(key.clone()).or_default();
            if stem == "mod" {
                entry.0 += 1;
            } else if path.parent().is_some_and(|dir| dir.join(stem).is_dir()) {
                entry.1 += 1;
            }
        }
    }

    let offenders: Vec<String> = styles
        .iter()
        .filter(|(_, (folder, sibling))| *folder > 0 && *sibling > 0)
        .filter(|(name, _)| !MIXED_MODULE_STYLE_ALLOWLIST.iter().any(|(known, _)| known == name))
        .map(|(name, (folder, sibling))| {
            format!("{name}: {folder} `mod.rs` against {sibling} sibling file(s)")
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "a crate that uses both module styles makes every reader guess which one a module \
         followed:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_allowlist_names_only_crates_that_still_exist() {
    let root = workspace_root();
    let stale: Vec<&str> = MIXED_MODULE_STYLE_ALLOWLIST
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !Path::new(&root).join(name).join("Cargo.toml").exists())
        .collect();

    assert!(stale.is_empty(), "these allowlist entries outlived their crates: {stale:?}");
}

// WHAT WAS MEASURED AND DELIBERATELY NOT MADE A RULE.
//
// The W0 specification also asked for a rule on test-file naming: a `#[cfg(test)]` module in a
// file whose name does not mark it as test code. Measured before writing: 206 source files carry
// an inline `#[cfg(test)] mod tests`, against 21 files named `tests.rs` or `*_tests.rs`.
//
// So the inline module is not the deviation — it is the convention, by ten to one, and it is
// idiomatic Rust besides. A rule flagging it would have shipped with a 206-entry allowlist, which
// is not a ratchet; it is a rename campaign wearing one. The same measurement is the argument
// against "N files sharing a prefix instead of a directory" and "a directory grouping on more than
// one axis": both are judgements about taste that a scan can only approximate, and an approximate
// rule spends its life being argued with rather than obeyed.
//
// One real consequence of the split convention IS worth writing down, because it is not a matter
// of taste: `duplication.rs` treats the two spellings differently. Its NAME-collision scan skips
// files named `tests.rs` / `*_tests.rs`, so those files dodge that rule purely by what they are
// called — while its IDENTICAL-BODY scan covers them like everything else (proof: entries from
// `*_tests.rs` files sit on `IDENTICAL_BODY_ALLOWLIST`). The asymmetry is recorded here rather
// than changed, because narrowing it belongs to the duplication gate's own ratchet.
