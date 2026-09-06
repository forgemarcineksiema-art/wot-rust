//! Architecture gate: the interface migrates off vertex-equality tests and off the legacy
//! primitives, and the count of each only goes DOWN.
//!
//! Interface program F5. The old HUD was locked by tests that found a feature by the exact
//! colour of its vertices (`theme::tagged` exists to make that possible), and drawn by calls to
//! `push_quad`, `push_panel`, `push_text` on a bare `Vec<HudVertex>`. Both are the cost of the
//! redesign: a restyled plate breaks the test that named its old colour, and a new panel on
//! the old primitives is a panel the draw list cannot see. So both are ratcheted per file:
//! a file may not exceed its ceiling, a file not listed may hold none, and a ceiling that
//! reaches zero must be deleted from the table (the stale check) — the way the layer rules burn
//! their allowlist. The reticle files are HELD: their locks are the A lane's and stay exactly
//! as they are until H27 moves them on purpose.

use std::fs;
use std::path::Path;

use quality::workspace::workspace_root;

#[derive(Clone, Copy, PartialEq)]
enum Hold {
    /// The count may only fall; at zero the entry is deleted.
    Burn,
    /// The count must stay exactly here (the reticle stack, H25).
    Held,
}

/// Lines asserting vertex-colour equality (`.color ==` / `.color !=`) per file. Seeded
/// 2026-09-05 from the tree.
const VERTEX_EQUALITY_CEILINGS: &[(&str, usize, Hold)] = &[
    ("crates/apps/client/src/hud/demo_strip.rs", 1, Hold::Held),
    ("crates/apps/client/src/hud/hit_direction.rs", 4, Hold::Burn),
    ("crates/apps/client/src/hud/minimap.rs", 2, Hold::Burn),
    ("crates/apps/client/src/hud/reticle_overlay_tests.rs", 42, Hold::Held),
    ("crates/apps/client/src/hud/scope_overlay.rs", 2, Hold::Held),
    ("crates/apps/client/src/hud/tests.rs", 16, Hold::Burn),
];

/// Direct calls to the legacy primitives (`push_quad(`, `push_panel(`, `push_text(`, …)
/// outside `ui_kit` and the reticle files, per file. Seeded 2026-09-05.
const LEGACY_CALL_SITE_CEILINGS: &[(&str, usize, Hold)] = &[
    ("crates/apps/client/src/hit_indicator.rs", 1, Hold::Burn),
    ("crates/apps/client/src/hit_indicator/draw.rs", 20, Hold::Burn),
    ("crates/apps/client/src/hud/demo_strip.rs", 2, Hold::Held),
    ("crates/apps/client/src/hud/hit_direction.rs", 2, Hold::Burn),
    ("crates/apps/client/src/hud/kill_marker.rs", 2, Hold::Burn),
    ("crates/apps/client/src/hud/minimap.rs", 3, Hold::Burn),
    ("crates/apps/client/src/hud/number.rs", 1, Hold::Burn),
    ("crates/apps/client/src/hud/outcome.rs", 3, Hold::Burn),
    ("crates/apps/client/src/hud/readouts.rs", 6, Hold::Burn),
    ("crates/apps/client/src/hud/reticle_marks.rs", 9, Hold::Held),
    ("crates/apps/client/src/hud/reticle_overlay.rs", 8, Hold::Held),
    ("crates/apps/client/src/hud/reticle_readouts.rs", 5, Hold::Held),
    ("crates/apps/client/src/hud/scope_overlay.rs", 2, Hold::Held),
    ("crates/apps/client/src/hud/tests.rs", 1, Hold::Burn),
];

const SCANNED_DIRS: &[&str] = &["crates/apps/client/src"];

/// Files whose counts are held rather than burned, by path fragment.
const HELD_FRAGMENTS: &[&str] = &["/hud/reticle", "/hud/demo_strip.rs", "/hud/scope_overlay.rs"];

const VERTEX_NEEDLES: &[&str] = &[".color ==", ".color !="];
const CALL_NEEDLES: &[&str] = &[
    "push_quad(",
    "push_panel(",
    "push_text(",
    "push_text_right(",
    "push_text_styled(",
    "push_text_right_styled(",
    "push_text_tabular(",
    "push_bar(",
    "push_arc(",
    "push_segment(",
    "push_hairline(",
    "push_icon(",
];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn count(source: &str, needles: &[&str]) -> usize {
    source.lines().filter(|line| needles.iter().any(|n| line.contains(n))).count()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn ratchet(needles: &[&str], ceilings: &[(&str, usize, Hold)], what: &str) {
    let root = workspace_root();
    let mut files = Vec::new();
    for dir in SCANNED_DIRS {
        rust_files(&root.join(dir), &mut files);
    }
    let mut offenders = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in files {
        let rel = relative(&root, &path);
        let source = fs::read_to_string(&path).unwrap_or_default();
        let found = count(&source, needles);
        let entry = ceilings.iter().find(|(file, _, _)| *file == rel);
        match entry {
            Some((_, ceiling, hold)) => {
                seen.insert(rel.clone());
                match hold {
                    Hold::Held if found != *ceiling => offenders.push(format!(
                        "{rel}: {what} count {found} moved from its held {ceiling} — the reticle stack is H25's and H27's, not this PR's"
                    )),
                    Hold::Burn if found > *ceiling => offenders.push(format!(
                        "{rel}: {what} count {found} exceeds the ceiling {ceiling} — migrate to the draw list instead of adding to the old kit"
                    )),
                    Hold::Burn if found == 0 => offenders.push(format!(
                        "{rel}: {what} count reached zero — delete its entry; the ratchet only burns down"
                    )),
                    _ => {}
                }
            }
            None if found > 0 => offenders.push(format!(
                "{rel}: {found} {what} in a file the ratchet does not list — new code goes through the draw list"
            )),
            None => {}
        }
    }
    for (file, _, _) in ceilings {
        if !seen.contains(*file) {
            offenders.push(format!(
                "{file}: listed in the ratchet but not in the tree — delete the entry"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "the {what} ratchet only goes down:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn vertex_colour_equality_assertions_only_go_down() {
    ratchet(VERTEX_NEEDLES, VERTEX_EQUALITY_CEILINGS, "vertex-colour-equality");
}

#[test]
fn legacy_primitive_call_sites_only_go_down() {
    ratchet(CALL_NEEDLES, LEGACY_CALL_SITE_CEILINGS, "legacy-primitive-call-site");
}

#[test]
fn the_held_files_are_the_reticle_stack() {
    for (file, _, hold) in VERTEX_EQUALITY_CEILINGS.iter().chain(LEGACY_CALL_SITE_CEILINGS) {
        let held_by_name = HELD_FRAGMENTS.iter().any(|f| file.contains(f));
        assert_eq!(
            *hold == Hold::Held,
            held_by_name,
            "{file}: a held entry must be a reticle-stack file and every reticle-stack file is held"
        );
    }
}
