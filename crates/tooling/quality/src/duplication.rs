//! Architecture gate: detect copy-pasted module-level free functions across crate `src` trees.
//!
//! Splitting a file is cheaper than sharing code, so the same leaf helper can end up pasted into
//! several crates. This scan closes that gap: it flags free functions defined under the same name
//! in more than one `src` module, pushing them into a shared crate (see `game_core::math`)
//! instead.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Module-level (column-0) free functions allowed to share a name across `src` modules.
///
/// Two buckets live here:
///   * intrinsically per-file — binary entry points and helpers that cannot move into the shared
///     `game_core` data crate without dragging in a heavier dependency;
///   * pre-existing copy-paste surfaced when this gate landed. These are tracked so the gate ships
///     green while still blocking *new* duplication. Burn the list down by hoisting the function
///     into a shared module and deleting its entry here.
pub const DUPLICATE_FREE_FN_ALLOWLIST: &[&str] = &[
    // Binary entry point — one per bin crate by definition.
    "main",
    // App-shell entry point (`client::run`, `editor::app::run`) — the windowed-app twin of
    // `main`: one per app by design, nothing shareable behind the name.
    "run",
    // One vehicle's component layout, and the hull half of it: `damage_layout::<vehicle>::layout`
    // is that module's entry point exactly as `main` is a binary's. The bodies place different
    // objects in different hulls and share nothing but the section heading — the arithmetic they
    // DID share (hull anchors, fuel cells, the turret group) has been hoisted into
    // `damage_layout::authoring`, which is what the name scan cannot see and the body scan can.
    "layout",
    "hull_components",
    // The ratchet is empty: every pre-existing duplicate has been hoisted into a shared home —
    // `rotate_around` / `armor_normal` / `world_to_tank_local` into `game_core::math`, and the
    // shell-collision helpers (`terrain_crossing`, `first_cover_impact`, the tank ray-AABB) into
    // `sim::shell_trace`, the single shell-physics implementation shared by server and client.
];

/// One human-readable line per offending duplicated free function (empty when the tree is clean).
pub fn duplicated_free_functions() -> Vec<String> {
    let root = workspace_root();
    let mut definitions: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();

    for src_dir in crate::workspace::crate_src_dirs(&root) {
        for path in rust_files(&src_dir) {
            if is_test_module_file(&path) {
                continue;
            }
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            for name in free_function_names(&source) {
                definitions.entry(name).or_default().insert(path.clone());
            }
        }
    }

    duplicate_offenders(&definitions, DUPLICATE_FREE_FN_ALLOWLIST, &root)
}

/// Names of module-level (column-0) free functions in a Rust source string. `impl`/trait methods
/// and functions nested in inline `mod` blocks are indented, so restricting to column 0 isolates
/// genuine free functions without parsing the full grammar.
pub fn free_function_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| !line.starts_with(char::is_whitespace))
        .filter_map(free_function_name)
        .collect()
}

/// The function name a single column-0 line defines, if it is a free-function definition.
pub fn free_function_name(line: &str) -> Option<String> {
    let mut saw_fn = false;
    for token in line.split_whitespace() {
        if saw_fn {
            let name: String = token.chars().take_while(|&c| c != '(' && c != '<').collect();
            let first = name.chars().next()?;
            return (first == '_' || first.is_ascii_lowercase()).then_some(name);
        }
        if token == "fn" {
            saw_fn = true;
        } else if token.starts_with("pub")
            || matches!(token, "async" | "const" | "unsafe" | "extern")
            || token.starts_with('"')
        {
            continue;
        } else {
            return None;
        }
    }
    None
}

/// Names defined in more than one file and not allowlisted, formatted with their locations.
pub fn duplicate_offenders(
    definitions: &BTreeMap<String, BTreeSet<PathBuf>>,
    allowlist: &[&str],
    root: &Path,
) -> Vec<String> {
    definitions
        .iter()
        .filter(|(name, files)| files.len() > 1 && !allowlist.contains(&name.as_str()))
        .map(|(name, files)| {
            let list = files
                .iter()
                .map(|path| path.strip_prefix(root).unwrap_or(path).display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("`{name}` defined in {} files: {list}", files.len())
        })
        .collect()
}

fn workspace_root() -> PathBuf {
    // Layout-agnostic: the nearest ancestor whose Cargo.toml declares [workspace].
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
    {
        assert!(dir.pop(), "a Cargo.toml with [workspace] should exist in an ancestor");
    }
    dir
}

/// `#[cfg(test)] mod tests` lives in files named `tests.rs` / `*_tests.rs`; their helper functions
/// are commonly (and harmlessly) duplicated, so the duplication gate skips them.
fn is_test_module_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_files(root, &mut paths);
    paths
}

fn collect_rust_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("source directory should be readable") {
        let path = entry.expect("source entry should be readable").path();
        if path.is_dir() {
            collect_rust_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_reads_column_zero_fns_and_ignores_other_lines() {
        assert_eq!(
            free_function_name("fn horizontal_forward(yaw_rad: f32) -> Vec3 {").as_deref(),
            Some("horizontal_forward")
        );
        assert_eq!(
            free_function_name("pub(crate) fn gun_direction(yaw: f32) -> Vec3 {").as_deref(),
            Some("gun_direction")
        );
        assert_eq!(
            free_function_name("pub const fn segment_box_entry<T>(p0: T) {").as_deref(),
            Some("segment_box_entry")
        );
        assert_eq!(free_function_name("struct Foo {"), None);
        assert_eq!(free_function_name("use game_core::math;"), None);

        // Indented lines (impl/trait methods, inline `mod` bodies) are dropped before parsing.
        let names =
            free_function_names("fn a() {}\n    fn method() {}\nstruct S;\npub fn b() {}\n");
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn offenders_flag_cross_file_dupes_but_respect_the_allowlist() {
        let definitions: BTreeMap<String, BTreeSet<PathBuf>> = [
            ("dup", vec!["a.rs", "b.rs"]),
            ("solo", vec!["a.rs"]),
            ("allowed", vec!["a.rs", "b.rs"]),
        ]
        .into_iter()
        .map(|(name, files)| (name.to_string(), files.into_iter().map(PathBuf::from).collect()))
        .collect();

        let offenders = duplicate_offenders(&definitions, &["allowed"], Path::new(""));

        assert_eq!(offenders.len(), 1, "only the non-allowlisted cross-file dup should flag");
        assert!(offenders[0].contains("dup"), "got: {offenders:?}");
    }
}
