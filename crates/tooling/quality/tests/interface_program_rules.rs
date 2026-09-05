//! Architecture gate: the interface program's register points at files that exist, and at
//! rows that are one of a kind.
//!
//! The second pass found that "documents lie in the details": a dossier quoting a cap the code
//! had raised, a policy marking bakes "done" that one kernel authored, a program row citing a
//! line that had moved. `docs/interface-program.md` is a register whose Evidence column is
//! nothing but paths — and a path is the one claim a test can read back. Every path an Evidence
//! cell names must exist in the tree; a file that is renamed, split or retired fails the gate in
//! the PR that moves it, so the register is corrected there rather than found stale by the next
//! reader. Register IDs must be unique and wear one of the program's four wave prefixes, because
//! other documents refer to them by name ("absorbs U6", "see H8").

use std::collections::HashSet;
use std::fs;

use quality::workspace::workspace_root;

const PROGRAM: &str = "docs/interface-program.md";

/// The wave prefixes the program's register is allowed to use.
const WAVES: [char; 4] = ['F', 'H', 'P', 'G'];

/// A register row is a table line whose first cell is a wave letter followed by digits.
fn register_rows(doc: &str) -> Vec<(String, Vec<String>)> {
    doc.lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('|') {
                return None;
            }
            let cells: Vec<String> =
                line.trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect();
            let id = cells.first()?.clone();
            let mut chars = id.chars();
            let wave = chars.next()?;
            let digits: String = chars.collect();
            let is_row = WAVES.contains(&wave)
                && !digits.is_empty()
                && digits.chars().all(|c| c.is_ascii_digit());
            is_row.then_some((id, cells))
        })
        .collect()
}

/// Every repo-relative path a cell names: a `crates/…`, `docs/…` or `assets/…` token, cut at
/// the first character that cannot be part of a path, with a trailing `:line-line` removed.
fn paths_in(cell: &str) -> Vec<String> {
    let mut found = Vec::new();
    for prefix in ["crates/", "docs/", "assets/"] {
        let mut rest = cell;
        while let Some(start) = rest.find(prefix) {
            let tail = &rest[start..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '.' | '-')))
                .unwrap_or(tail.len());
            let token = tail[..end].trim_end_matches('.');
            let token = token.split(':').next().unwrap_or(token);
            if !token.is_empty() {
                found.push(token.to_string());
            }
            rest = &tail[end.max(prefix.len())..];
        }
    }
    found
}

#[test]
fn every_evidence_path_in_the_interface_program_exists() {
    let root = workspace_root();
    let doc = fs::read_to_string(root.join(PROGRAM))
        .unwrap_or_else(|_| panic!("{PROGRAM} should be readable"));

    let rows = register_rows(&doc);
    assert!(!rows.is_empty(), "{PROGRAM} should carry a register with F/H/P/G rows");

    let mut offenders = Vec::new();
    let mut checked = 0usize;
    for (id, cells) in &rows {
        // Columns: ID | Defect | Evidence | Wave | Closes when. Only the Evidence cell is a
        // claim about the tree; "Closes when" names files that do not exist yet on purpose.
        let Some(evidence) = cells.get(2) else {
            offenders.push(format!("{id}: the row has no Evidence cell"));
            continue;
        };
        for path in paths_in(evidence) {
            checked += 1;
            if !root.join(&path).exists() {
                offenders.push(format!("{id}: `{path}` does not exist in the tree"));
            }
        }
    }

    assert!(checked > 0, "{PROGRAM}: no Evidence cell names a path — the gate reads nothing");
    assert!(
        offenders.is_empty(),
        "{PROGRAM} cites evidence the tree no longer holds — move the register with the file:\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn every_register_id_in_the_interface_program_is_unique() {
    let root = workspace_root();
    let doc = fs::read_to_string(root.join(PROGRAM))
        .unwrap_or_else(|_| panic!("{PROGRAM} should be readable"));

    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    for (id, _) in register_rows(&doc) {
        if !seen.insert(id.clone()) {
            duplicates.push(id);
        }
    }

    assert!(
        duplicates.is_empty(),
        "{PROGRAM}: a register ID appears twice, and other documents refer to rows by ID: {}",
        duplicates.join(", "),
    );
    for wave in WAVES {
        assert!(
            seen.iter().any(|id| id.starts_with(wave)),
            "{PROGRAM}: wave {wave} has no rows — the four-wave shape this gate guards has changed",
        );
    }
}

#[test]
fn a_path_is_read_the_way_a_register_cell_writes_it() {
    assert_eq!(
        paths_in("`crates/apps/client/src/hud.rs:175-264`, `crates/ui/ui_kit/src/theme.rs`"),
        ["crates/apps/client/src/hud.rs", "crates/ui/ui_kit/src/theme.rs"],
    );
    assert_eq!(paths_in("`docs/game-design.md` (row 13)."), ["docs/game-design.md"]);
    assert_eq!(paths_in("the `crates/ui/ui_kit/src/` tree"), ["crates/ui/ui_kit/src/"]);
    assert!(paths_in("nothing here").is_empty());
}
