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

/// H24: the only full-screen scrim the battle HUD knows is the escape menu's. A quad with
/// clip half-extents `[1.0, 1.0]` covers the whole viewport; outside `pause_menu.rs` nothing
/// in `hud/` may push one — a popup in battle is a rule broken, not a feature.
#[test]
fn nothing_modal_appears_in_battle_but_the_escape_menu() {
    let root = workspace_root();
    let hud = root.join("crates/apps/client/src/hud");
    let mut offenders = Vec::new();
    for entry in fs::read_dir(&hud).expect("hud dir").flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name == "pause_menu.rs" || name.ends_with("tests.rs") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap_or_default();
        let code = source.split("#[cfg(test)]").next().unwrap_or("");
        for (line_number, line) in code.lines().enumerate() {
            if line.contains("push_quad(") && line.contains("[1.0, 1.0]") {
                offenders.push(format!("hud/{name}:{}: a full-screen quad", line_number + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "nothing modal appears in battle but the escape menu:{}  {}",
        '\n',
        offenders.join("\n  ")
    );
}

/// P7: the hard-coded key match is gone. Every key the app reads is the table's word
/// (`app/keybinds.rs`); no other file of the app names a `KeyCode` outside its tests.
#[test]
fn the_hard_coded_key_match_is_gone() {
    let root = workspace_root();
    let app = root.join("crates/apps/client/src/app");
    let mut files = Vec::new();
    rust_files_under(&app, &mut files);
    let mut offenders = Vec::new();
    for path in files {
        let rel = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        if rel.ends_with("/keybinds.rs") || rel.ends_with("tests.rs") {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap_or_default();
        let code = source.split("#[cfg(test)]").next().unwrap_or("");
        for (line_number, line) in code.lines().enumerate() {
            if line.contains("KeyCode::") && !line.trim_start().starts_with("//") {
                offenders.push(format!("{rel}:{}: {}", line_number + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the keys are the table's (app/keybinds.rs); a key named elsewhere is a binding the player cannot change:{}  {}",
        '\n',
        offenders.join("\n  ")
    );
}

fn rust_files_under(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// P9: borderless is the only fullscreen — no `Fullscreen::Exclusive` anywhere in the client
/// (a mode switch, a black flash on alt-tab, a fight with the compositor's vsync) — and the
/// window takes the SETTING, not a flag of its own: F11 writes `settings.json`, the window
/// follows it the moment it exists.
#[test]
fn borderless_is_the_only_fullscreen_and_it_is_a_setting() {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_files_under(&root.join("crates/apps/client/src"), &mut files);
    let mut exclusive = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path).unwrap_or_default();
        if source.contains("Fullscreen::Exclusive") {
            exclusive.push(
                path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().replace('\\', "/"),
            );
        }
    }
    assert!(exclusive.is_empty(), "exclusive fullscreen is not on offer: {exclusive:?}");
    let settings =
        fs::read_to_string(root.join("crates/apps/client/src/app/settings.rs")).expect("settings");
    assert!(settings.contains("pub borderless: bool"), "borderless is a setting");
    assert!(
        settings.contains("Fullscreen::Borderless(None)"),
        "the setting is what the window takes"
    );
    let lifecycle = fs::read_to_string(root.join("crates/apps/client/src/app/lifecycle.rs"))
        .expect("lifecycle");
    assert!(
        lifecycle.contains("self.apply_fullscreen_setting();"),
        "the window takes the setting when it exists"
    );
    let input =
        fs::read_to_string(root.join("crates/apps/client/src/app/input.rs")).expect("input");
    assert!(
        input.contains("settings.borderless = borderless"),
        "F11 writes the setting, not a flag of its own"
    );
}
