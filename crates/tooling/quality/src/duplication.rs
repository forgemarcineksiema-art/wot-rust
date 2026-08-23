//! Architecture gate: detect copy-pasted free functions across the workspace.
//!
//! Splitting a file is cheaper than sharing code, so the same leaf helper ends up pasted into
//! several places. Two scans, deliberately different in strictness:
//!
//!   * by NAME, across crate `src` trees — the original gate, pushing shared leaves into a shared
//!     crate (see `game_core::math`);
//!   * by BODY, across `src`, `tests`, `examples` and `benches` — the same function character for
//!     character in two files, which is one edit that will only ever land in one of them.
//!
//! The name scan cannot reach test trees: two crates both calling a local fixture `hull` is a
//! coincidence, not duplication. The body scan can, because identical code is never a
//! coincidence.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace::{is_test_module_file, rust_files, workspace_root};

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

/// Functions with byte-identical bodies that are duplicated ON PURPOSE, with the reason.
///
/// Deliberately separate from [`DUPLICATE_FREE_FN_ALLOWLIST`]: that one forgives a shared NAME,
/// which is often just two crates calling a local fixture `hull`. This one forgives shared CODE,
/// which is a much stronger claim and needs a much better excuse.
pub const IDENTICAL_BODY_ALLOWLIST: &[&str] = &[
    // The 2026-08-23 burn hoisted seventeen entries into `tests/common` modules (or a shared
    // home: `game_core::math::srgb_to_linear`, `review_views::map_key`) and split the helpers
    // that shared a name while doing different things (`run_until_shells_clear`, `luma_255`).
    // These two remain, each with the reason it cannot take the same route:
    //
    // The T-54 LOD fixture is duplicated between `vehicle_build`'s `tests/quality.rs` and
    // `examples/assemble.rs` — an example cannot see `tests/common`, and a test fixture is not
    // lib API. Burns if the example ever composes the bake differently.
    "lod_asset",
    // Same-shaped convenience over two DIFFERENT per-module `snapshot_at` fixtures in
    // `client/src/predict/{tests,interpolation_tests}.rs`; one shared body would silently
    // couple fixtures that are meant to diverge. Burns if the fixtures themselves unify.
    "snapshot_with_aim",
];

/// The same function, character for character, in more than one file.
///
/// Sharper than the name-collision scan and aimed at a different failure: two crates both calling
/// a local fixture `hull` is a coincidence, but two files holding the same fourteen lines is one
/// edit that will only ever land in one of them.
///
/// Bodies are compared with comments stripped and whitespace collapsed, so re-indenting or
/// re-wrapping a comment does not hide a copy. Functions under 60 characters of body are ignored —
/// a one-line getter that happens to match is not a maintenance hazard.
pub fn identical_function_bodies() -> Vec<String> {
    duplicate_offenders(&identical_body_map(), IDENTICAL_BODY_ALLOWLIST, &workspace_root())
}

/// Names that still have at least two byte-identical bodies somewhere in the tree — the live
/// facts [`IDENTICAL_BODY_ALLOWLIST`] is allowed to describe. An entry outside this set is
/// stale: the duplication it forgave is gone.
pub fn names_with_identical_bodies() -> BTreeSet<String> {
    identical_body_map().into_keys().collect()
}

/// Every function name mapped to the files holding an identical copy (only names where some
/// normalized body appears in more than one file).
fn identical_body_map() -> BTreeMap<String, BTreeSet<PathBuf>> {
    let root = workspace_root();
    let mut bodies: BTreeMap<(String, String), BTreeSet<PathBuf>> = BTreeMap::new();

    for dir in scanned_dirs(&root) {
        for path in rust_files(&dir) {
            let source = fs::read_to_string(&path).expect("Rust source should be readable");
            for (name, body) in free_functions_with_bodies(&source) {
                bodies.entry((name, body)).or_default().insert(path.clone());
            }
        }
    }

    let mut offenders: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
    for ((name, _), files) in bodies.into_iter().filter(|(_, files)| files.len() > 1) {
        offenders.entry(name).or_default().extend(files);
    }
    offenders
}

/// `src`, `tests`, `examples` and `benches` of every workspace crate.
///
/// The original scan saw only `src`, which is where copy-paste is least likely: a helper worth
/// sharing between two crates' libraries tends to get shared. Test trees are where it accumulates,
/// because two integration tests cannot see each other's code without someone deciding to make a
/// module for it.
fn scanned_dirs(root: &Path) -> Vec<PathBuf> {
    crate::workspace::crate_facts(root)
        .into_iter()
        .flat_map(|krate| ["src", "tests", "examples", "benches"].map(|sub| krate.dir.join(sub)))
        .filter(|dir| dir.is_dir())
        .collect()
}

/// Column-0 free functions paired with their normalized body. `#[test]`/`#[bench]` functions are
/// skipped: two tests asserting the same thing about different subjects is not duplication, and
/// two identical tests are a different (and rarer) problem.
fn free_functions_with_bodies(source: &str) -> Vec<(String, String)> {
    // Byte offsets come from `split_inclusive`, which keeps the line terminator. `lines()` drops a
    // `\r\n` down to `\n`, so accumulating `len() + 1` drifts one byte per line on this repo's
    // CRLF checkouts and eventually indexes into the middle of a multi-byte character.
    let mut lines = Vec::new();
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        lines.push((offset, line.trim_end_matches(['\n', '\r'])));
        offset += line.len();
    }

    let mut found = Vec::new();
    for (index, (start, line)) in lines.iter().enumerate() {
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some(name) = free_function_name(line) else { continue };
        let attributed = lines[index.saturating_sub(3)..index]
            .iter()
            .any(|(_, above)| above.contains("#[test]") || above.contains("#[bench]"));
        if attributed {
            continue;
        }
        let Some(body) = braced_body(&source[*start..]) else { continue };
        if body.len() >= 60 {
            found.push((name, body));
        }
    }
    found
}

/// The `{ .. }` body starting at the first brace, comments stripped and whitespace collapsed.
fn braced_body(text: &str) -> Option<String> {
    let open = text.find('{')?;
    let mut depth = 0usize;
    let mut end = None;
    for (at, byte) in text[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(open + at);
                    break;
                }
            }
            _ => {}
        }
    }
    let body = &text[open..=end?];
    let stripped: String = body
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default().trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(stripped)
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
