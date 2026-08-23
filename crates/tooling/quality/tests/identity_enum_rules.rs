//! The enums that ARE the wire and the assets, and the constant that has to keep listing all of
//! them.
//!
//! CLAUDE.md calls these append-only: the variant order is a stored identity, so `MapId`,
//! `SceneryKind`, `VehicleKind` and their siblings can gain a variant but never lose or reorder
//! one. Every coverage test in this repo — armour zones, shader material ids, map goldens —
//! iterates an `ALL` constant to ask "is each one handled?". That question is only as honest as
//! `ALL` is complete, and nothing was checking that.
//!
//! `VehicleKind::ALL` was locked by `assert_eq!(VehicleKind::ALL.len(), 8)`. Add a ninth variant
//! and forget the constant, and the length is still 9 and the assertion still passes: the lock
//! held the number, not the roster. This file reads both the enum body and the constant and
//! compares them name by name.

use quality::workspace_root;
use std::path::{Path, PathBuf};

/// Enums whose variant set is an identity — stored in blueprints, on the wire, or in a baked
/// asset — and which therefore must carry a complete `ALL`.
///
/// This list is checked against the sources: an entry naming an enum that no longer exists fails,
/// so the register cannot rot into a list of good intentions.
const IDENTITY_ENUMS: &[&str] = &[
    "ArmorZone",
    "DamageCause",
    "MapId",
    "MaterialRole",
    "ModuleSlot",
    "Nation",
    "Penetrator",
    "RoadSurface",
    "RoundId",
    "SceneryKind",
    "ShellType",
    "StaticCoverKind",
    "VehicleKind",
];

#[test]
fn every_identity_enum_carries_an_all_constant() {
    let sources = rust_sources(&workspace_root());
    let mut missing = Vec::new();

    for name in IDENTITY_ENUMS {
        let Some((path, text)) = sources.iter().find(|(_, text)| declares_enum(text, name)) else {
            missing.push(format!("`{name}` is in IDENTITY_ENUMS but declared nowhere"));
            continue;
        };
        if all_constant(text, name).is_none() {
            missing.push(format!("{}: `{name}` has no `pub const ALL`", path.display()));
        }
    }

    assert!(
        missing.is_empty(),
        "an append-only enum with no ALL is an enum no coverage test can walk:\n  {}",
        missing.join("\n  ")
    );
}

/// The lock the length assertions could not provide.
#[test]
fn every_all_constant_names_every_variant() {
    let sources = rust_sources(&workspace_root());
    let mut offenders = Vec::new();

    for (path, text) in &sources {
        for name in enums_declared_in(text) {
            let Some(listed) = all_constant(text, &name) else { continue };
            let declared = variants_of(text, &name);
            let forgotten: Vec<_> =
                declared.iter().filter(|variant| !listed.contains(*variant)).collect();
            let invented: Vec<_> =
                listed.iter().filter(|variant| !declared.contains(*variant)).collect();

            if !forgotten.is_empty() {
                offenders.push(format!("{}: {name}::ALL is missing {forgotten:?}", path.display()));
            }
            if !invented.is_empty() {
                offenders.push(format!(
                    "{}: {name}::ALL names {invented:?}, which are not variants of it",
                    path.display()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "an ALL that is not all makes every test that walks it quietly partial:\n  {}",
        offenders.join("\n  ")
    );
}

/// `ALL` has to mean the same thing everywhere or the rule above means nothing. `MapId::ALL` used
/// to hold only the SHIPPED maps — `ALL.contains(&Scratch)` was false — while `VehicleKind` next
/// door used `ALL` for the complete set and `PLAYABLE` for the subset. A subset gets a name that
/// says which subset.
#[test]
fn no_all_constant_is_secretly_a_subset() {
    // Enforced by `every_all_constant_names_every_variant`; this test states the naming rule for a
    // reader and fails loudly if the sibling constant that made room for it disappears.
    let map_id =
        std::fs::read_to_string(workspace_root().join("crates/foundation/terrain/src/map_id.rs"))
            .expect("map_id.rs is readable");

    assert!(
        map_id.contains("pub const SHIPPED"),
        "MapId's shipped-map subset must keep a name that says it is a subset"
    );
}

/// Every `pub enum Name {` in a file.
fn enums_declared_in(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("pub enum ") {
        let after = &rest[at + "pub enum ".len()..];
        let name: String = after.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && after[name.len()..].trim_start().starts_with('{') {
            names.push(name);
        }
        rest = after;
    }
    names
}

fn declares_enum(text: &str, name: &str) -> bool {
    enums_declared_in(text).iter().any(|declared| declared == name)
}

/// Variant names from the enum body: the identifiers at one level of indentation inside it.
fn variants_of(text: &str, name: &str) -> Vec<String> {
    let Some(at) = text
        .find(&format!("pub enum {name} "))
        .or_else(|| text.find(&format!("pub enum {name}\n")))
    else {
        return Vec::new();
    };
    let Some(open) = text[at..].find('{').map(|offset| at + offset) else { return Vec::new() };
    let body = braced(&text[open..]);

    let mut variants = Vec::new();
    let mut depth = 0usize;
    for line in body.lines() {
        let trimmed = line.trim();
        if depth == 0 && !trimmed.starts_with("//") && !trimmed.starts_with('#') {
            let ident: String =
                trimmed.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let tail = trimmed[ident.len()..].trim_start();
            let starts_variant = ident.starts_with(|c: char| c.is_ascii_uppercase())
                && (tail.starts_with(',')
                    || tail.starts_with('{')
                    || tail.starts_with('(')
                    || tail.starts_with('=')
                    || tail.is_empty());
            if starts_variant {
                variants.push(ident);
            }
        }
        // Struct and tuple variants nest; a variant only counts at the body's own level.
        let opened = trimmed.matches(['{', '(']).count();
        let closed = trimmed.matches(['}', ')']).count();
        depth = (depth + opened).saturating_sub(closed);
    }
    variants
}

/// Variant names listed in `pub const ALL` for this enum, if it has one.
fn all_constant(text: &str, name: &str) -> Option<Vec<String>> {
    let marker = text.match_indices("pub const ALL:").find(|(at, _)| {
        let head: String = text[*at..].chars().take(160).collect();
        head.contains(&format!("[{name};")) || head.contains(&format!("[{name}]"))
    })?;
    let open = text[marker.0..].find('[').map(|offset| marker.0 + offset)?;
    let body = braced(&text[text[open..].find('=').map(|offset| open + offset)?..]);
    Some(
        body.split(&format!("{name}::"))
            .skip(1)
            .map(|piece| piece.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect())
            .collect(),
    )
}

/// The text between the first bracket of `text` and its match, exclusive.
fn braced(text: &str) -> &str {
    let bytes = text.as_bytes();
    let Some(open) = bytes.iter().position(|byte| matches!(byte, b'{' | b'[')) else { return "" };
    let mut depth = 0usize;
    for (offset, byte) in bytes[open..].iter().enumerate() {
        match byte {
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return &text[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    ""
}

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    for dir in quality::crate_src_dirs(root) {
        collect(&dir, &mut sources);
    }
    sources
}

fn collect(dir: &Path, sources: &mut Vec<(PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            sources.push((path, text));
        }
    }
}
