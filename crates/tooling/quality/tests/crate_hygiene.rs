//! Two ratchets on the shape of the workspace itself: a crate nobody uses does not linger, and a
//! dependency is declared in one place.
//!
//! Both are cheap to state and were both violated, quietly, for a long time — which is the point.
//! Nothing here is clever; it is the kind of thing a human reviewer stops noticing by the fortieth
//! manifest.

use quality::workspace_root;
use quality::{crate_facts, crate_manifests};

/// Crates with no dependents and no binary that are kept ON PURPOSE. Each entry is a decision;
/// an empty list is the goal.
const ORPHAN_ALLOWLIST: &[&str] = &[
    // `quality` IS the gate. Nothing depends on a test-only architecture crate by design.
    "quality",
    // Row 28 of docs/procedural-kernel-program.md: two finished, contract-tested kernels for thin
    // fabricated parts, built before the part that needs them. That is a real debt, not a neutral
    // fact — a capability nobody has used is a capability nobody has proven at the call site — and
    // this list is where it stops being invisible. They leave when a part calls them, or when the
    // program says the technique is not wanted.
    "panel",
    "shell",
    // Row 31: the sanctioned empty slot for a bake-only CAD trial. It holds no capability, only the
    // rule for opening and closing a trial without breaking `--all-features` for the workspace.
    "experimental_geometry",
];

/// A crate that nothing depends on and that produces no program is dead weight: it compiles on
/// every build, lints on every gate, and answers to nobody.
///
/// Tests do not justify a crate — a crate whose only consumers are its own tests is a crate
/// testing itself. Neither do examples: `sdf_mesh`'s T-54 metaball path survived exactly that way,
/// keeping a retired approach alive through two examples and a bench.
#[test]
fn no_crate_is_kept_alive_only_by_its_own_tests() {
    let facts = crate_facts(&workspace_root());
    let depended_on: Vec<&str> =
        facts.iter().flat_map(|krate| krate.deps.iter().map(String::as_str)).collect();

    let orphans: Vec<_> = facts
        .iter()
        .filter(|krate| {
            !depended_on.contains(&krate.name.as_str())
                && !krate.dir.join("src/main.rs").is_file()
                && !ORPHAN_ALLOWLIST.contains(&krate.name.as_str())
        })
        .map(|krate| format!("{} (crates/{})", krate.name, krate.layer))
        .collect();

    assert!(
        orphans.is_empty(),
        "nothing depends on these and they build no program — delete them, or give them a \
         consumer, or say in ORPHAN_ALLOWLIST why they stay:\n  {}",
        orphans.join("\n  ")
    );
}

/// crates.io dependencies declared outside the workspace table. Empty, and meant to stay empty:
/// `png` was declared inline in eight manifests and `ab_glyph` in one, all hoisted with this rule.
const INLINE_VERSION_ALLOWLIST: &[(&str, &str)] = &[];

/// The root manifest says why: "Paths are centralized here so a crate directory move only edits
/// this one block." Moving `terrain` from `runtime` to `foundation` cost exactly one line, which
/// is the promise being kept — and it only works while every crate honours it.
#[test]
fn dependencies_are_declared_through_the_workspace_table() {
    let mut offenders = Vec::new();
    for manifest in crate_manifests(&workspace_root()) {
        let name = manifest
            .parent()
            .and_then(|dir| dir.file_name())
            .map(|dir| dir.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&manifest).expect("manifest is readable");
        let mut section = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                section = trimmed.to_string();
                continue;
            }
            if !section.ends_with("dependencies]") || trimmed.starts_with('#') {
                continue;
            }
            let Some((dep, rest)) = trimmed.split_once('=') else { continue };
            let dep = dep.trim();
            if dep.is_empty() || dep.contains('.') {
                continue;
            }
            let rest = rest.trim();
            let inline_version = rest.starts_with('"');
            let raw_path = rest.contains("path =");
            if (inline_version && !INLINE_VERSION_ALLOWLIST.contains(&(name.as_str(), dep)))
                || raw_path
            {
                offenders.push(format!("{name}: `{trimmed}`"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "declare these in the root [workspace.dependencies] and use `name.workspace = true`, so \
         one version and one path exist per dependency:\n  {}",
        offenders.join("\n  ")
    );
}

/// The other half of centralizing dependencies: an entry nobody claims. A third-party crate listed
/// in the workspace table but declared by no member still pins a version, still shows up in a
/// dependency audit, and still reads as "we use this" to whoever comes next.
///
/// Only crates.io entries are checked. A path entry for a deliberate orphan (see
/// `ORPHAN_ALLOWLIST`) is unused on purpose and answers to that list instead.
#[test]
fn the_workspace_table_has_no_entry_nobody_uses() {
    let root = workspace_root();
    let text = std::fs::read_to_string(root.join("Cargo.toml")).expect("root manifest is readable");
    let table = text
        .split_once("[workspace.dependencies]")
        .and_then(|(_, rest)| rest.split_once("\n["))
        .map(|(table, _)| table)
        .expect("the root manifest has a [workspace.dependencies] table");

    let declared: Vec<&str> = table
        .lines()
        .filter(|line| !line.trim().starts_with('#') && !line.contains("path ="))
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.trim())
        .filter(|name| !name.is_empty() && !name.contains('.'))
        .collect();

    let members: Vec<String> = crate_manifests(&root)
        .iter()
        .filter_map(|manifest| std::fs::read_to_string(manifest).ok())
        .collect();

    let unclaimed: Vec<_> = declared
        .iter()
        .filter(|dep| {
            !members.iter().any(|manifest| {
                manifest.lines().map(str::trim).any(|line| {
                    line.starts_with(&format!("{dep} =")) || line.starts_with(&format!("{dep}."))
                })
            })
        })
        .collect();

    assert!(
        unclaimed.is_empty(),
        "declared in [workspace.dependencies] and used by no crate — delete the line: {unclaimed:?}"
    );
}

/// A manifest that does not inherit `version` and `rust-version` drifts from the workspace on its
/// own schedule, and nobody finds out until a release.
#[test]
fn every_manifest_inherits_the_workspace_version_and_toolchain() {
    let mut offenders = Vec::new();
    for manifest in crate_manifests(&workspace_root()) {
        let text = std::fs::read_to_string(&manifest).expect("manifest is readable");
        let name = manifest.parent().and_then(|dir| dir.file_name()).unwrap_or_default();
        for field in ["version.workspace", "rust-version.workspace", "edition.workspace"] {
            if !text.contains(field) {
                offenders.push(format!("{}: missing `{field} = true`", name.to_string_lossy()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these manifests set (or omit) their own where the workspace should answer:\n  {}",
        offenders.join("\n  ")
    );
}

/// An allowlist that outlives its exception is worse than no allowlist: it reads as a considered
/// decision while naming something that no longer exists. Both lists above have to earn their keep.
#[test]
fn the_allowlists_name_only_crates_that_still_exist() {
    let facts = crate_facts(&workspace_root());
    let known: Vec<&str> = facts.iter().map(|krate| krate.name.as_str()).collect();

    let stale: Vec<_> = ORPHAN_ALLOWLIST
        .iter()
        .chain(INLINE_VERSION_ALLOWLIST.iter().map(|(krate, _)| krate))
        .filter(|name| !known.contains(*name))
        .collect();

    assert!(stale.is_empty(), "allowlisted crates that no longer exist — drop them: {stale:?}");
}
