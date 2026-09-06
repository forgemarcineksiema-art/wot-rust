//! The roundness law is a RULE, not a habit: a revolved round part derives its segment count from
//! its radius (`game_core::roundness::round_segments` / `segments_for_radius`), never a hand-typed
//! integer that facets the part at some fixed count regardless of how large it is.
//!
//! This gate exists because the law was applied once and then drifted. The 2026-08-08 "roundness
//! re-record" migrated the SHARED deck helpers to `round_segments` but left hand-typed
//! `segments: 10` on the Tiger I stacks and the Tiger II / Panther II bespoke exhausts — a good
//! decision applied once instead of becoming a rule (the fix-as-rule antipattern). Nothing caught
//! the drift for months. Now a hand-typed segment count on a revolved part fails HERE, at the
//! authoring layer, the moment it is written.

use quality::workspace::repo_relative;
use quality::{rust_files, workspace_root};
use std::path::Path;

/// The vehicle authoring layers — where `RevolveSpec`s are written. Kernels below define the
/// revolve primitive; the segment COUNT is an authoring choice and belongs here.
const AUTHORING_SRC: &[&str] =
    &["crates/vehicle/vehicle_recipes/src", "crates/vehicle/vehicle_build/src"];

/// Hand-typed segment counts that are DELIBERATE, each with the measurement that justifies it.
/// An allowlist is a record of exceptions, not permission to add more — burn entries down, never
/// widen one. Keyed by (repo-relative file, a stable needle on the offending line).
const HARDCODED_SEGMENT_ALLOWLIST: &[(&str, &str, &str)] = &[
    // The DShK's barrel: max radius 0.026 m, so 10 segments facet at 1.27 mm — already inside the
    // 2.8 mm silhouette tolerance. `round_segments(0.026)` would give FEWER (8); hand-typing 10
    // over-tessellates a tiny part rather than faceting it. Not a defect, and a rare case where the
    // law would coarsen rather than refine.
    (
        "crates/vehicle/vehicle_build/src/t54_dshk.rs",
        "barrel_segments:",
        "r=0.026 m -> 1.27 mm facet, within the 2.8 mm tolerance; the law would give fewer",
    ),
];

/// One offending hand-typed segment count.
struct Offender {
    file: String,
    line: usize,
    text: String,
}

/// Every `segments:`-family field assigned a bare integer literal in the authoring source, OUTSIDE
/// a `#[cfg(test)]` module. Test fixtures set a fixed segment count on purpose (a `GunPlan` stub is
/// not a shipped part), and this repo keeps its `#[cfg(test)]` module last in the file, so scanning
/// stops at the first `#[cfg(test)]` line.
fn hardcoded_segment_offenders() -> Vec<Offender> {
    let root = workspace_root();
    let mut offenders = Vec::new();
    for dir in AUTHORING_SRC {
        for path in rust_files(&root.join(dir)) {
            let Ok(source) = std::fs::read_to_string(&path) else { continue };
            let rel = repo_relative(&path, &root);
            for (index, line) in source.lines().enumerate() {
                if line.contains("#[cfg(test)]") {
                    break; // the rest of the file is tests
                }
                if line_hardcodes_segments(line) && !is_allowlisted(&rel, line) {
                    offenders.push(Offender {
                        file: rel.clone(),
                        line: index + 1,
                        text: line.trim().to_string(),
                    });
                }
            }
        }
    }
    offenders
}

/// A `segments:` (or `barrel_segments:` …) field whose value begins with a digit. `round_segments(…)`
/// and `segments: n` (a variable) both fail this — only a bare literal like `segments: 10` matches.
fn line_hardcodes_segments(line: &str) -> bool {
    let Some(pos) = line.find("segments:") else { return false };
    let rest = line[pos + "segments:".len()..].trim_start();
    rest.starts_with(|c: char| c.is_ascii_digit())
}

fn is_allowlisted(rel: &str, line: &str) -> bool {
    HARDCODED_SEGMENT_ALLOWLIST
        .iter()
        .any(|(file, needle, _reason)| rel == *file && line.contains(needle))
}

#[test]
fn no_revolved_part_hand_types_its_segment_count() {
    let offenders = hardcoded_segment_offenders();
    assert!(
        offenders.is_empty(),
        "a revolved round part must derive its segment count from its radius \
         (`round_segments(r)` / `segments_for_radius(r, tol)`), not a hand-typed integer that \
         facets it at a fixed count. Fix these — or, if the count is deliberate, add a measured \
         allowlist entry:\n{}",
        offenders
            .iter()
            .map(|o| format!("  {}:{}  {}", o.file, o.line, o.text))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// An allowlist entry that no longer describes a real hand-typed count is stale permission: the
/// hardcode was removed (migrated to the law), so the entry must go too, or it silently forgives
/// the next one that lands on that file+needle. Same burn-down discipline as the other gates.
#[test]
fn the_hardcoded_segment_allowlist_has_no_stale_entries() {
    let root = workspace_root();
    let stale: Vec<&str> = HARDCODED_SEGMENT_ALLOWLIST
        .iter()
        .filter(|(file, needle, _)| !allowlist_entry_matches(&root, file, needle))
        .map(|(file, _, _)| *file)
        .collect();
    assert!(
        stale.is_empty(),
        "these allowlist entries match no hand-typed segment count any more — the hardcode is \
         gone, so delete the entry and let the rule protect the file again: {stale:?}"
    );
}

fn allowlist_entry_matches(root: &Path, file: &str, needle: &str) -> bool {
    let Ok(source) = std::fs::read_to_string(root.join(file)) else { return false };
    source
        .lines()
        .take_while(|line| !line.contains("#[cfg(test)]"))
        .any(|line| line.contains(needle) && line_hardcodes_segments(line))
}
