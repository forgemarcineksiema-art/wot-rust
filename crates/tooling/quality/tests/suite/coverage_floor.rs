//! Architecture gate: a test that walks the fleet must say how much of it it actually checked.
//!
//! The shape is everywhere in this repo and it is invisible by construction:
//!
//! ```ignore
//! for kind in VehicleKind::PLAYABLE {
//!     let Some(data) = source(kind) else { continue };
//!     assert!(something_about(data));
//! }
//! ```
//!
//! Green means "every vehicle that had data passed" — never "every vehicle had data". If the
//! source starts returning `None`, the loop checks nothing, the test passes, and the report says
//! the fleet is fine. `DamageLayout::for_vehicle` did exactly that: seven of eight vehicles fell
//! through a `_ =>` to an empty layout, and every test iterating the fleet stayed green because
//! each one skipped what it could not find.
//!
//! The fix is one line per loop, and `spaced_armor.rs` had it right all along: count what you
//! examined, then assert the count. `assert!(skirted >= 2, …)` is the whole rule — a floor that
//! fails when the data disappears, instead of a silence that looks like success.

use std::fs;

use quality::workspace::{integration_test_files, repo_relative, workspace_root};

/// The lists that name an identity in full. A loop over one of these is a claim about the fleet.
const IDENTITY_LISTS: &[&str] = &["ALL", "PLAYABLE", "SHIPPED"];

/// Skipping loops allowed to run without a coverage floor, with the reason.
///
/// Not a parking lot. An entry belongs here only when the loop's own assertions cannot go vacuous
/// — for instance when the body asserts something about the ABSENCE it skipped on.
const NO_FLOOR_ALLOWLIST: &[(&str, &str)] = &[(
    "crates/apps/tools/tests/studio_goldens.rs",
    "The loop's skip is the golden set itself: a vehicle with no recorded tiles is not a \
         silent pass, it is a vehicle the picture gate has not been pointed at yet, and \
         `the_golden_tree_has_no_orphans` covers the other direction.",
)];

#[test]
fn every_fleet_walk_asserts_how_much_of_the_fleet_it_walked() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for path in integration_test_files(&root) {
        let relative = repo_relative(&path, &root);
        if NO_FLOOR_ALLOWLIST.iter().any(|(file, _)| *file == relative) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("test source should be readable");
        for loop_site in skipping_loops(&source) {
            if !loop_site.has_floor {
                offenders.push(format!(
                    "{relative}:{} walks `{}` and can `continue` past a vehicle without counting \
                     what it checked — if the data vanished this test would pass having checked \
                     nothing",
                    loop_site.line, loop_site.list,
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a fleet walk with no floor is a test that reports on whatever happened to exist:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_allowlist_names_only_files_that_still_exist() {
    let root = workspace_root();
    let present: Vec<String> =
        integration_test_files(&root).iter().map(|path| repo_relative(path, &root)).collect();
    let stale: Vec<&str> = NO_FLOOR_ALLOWLIST
        .iter()
        .map(|(file, _)| *file)
        .filter(|file| !present.iter().any(|found| found == file))
        .collect();

    assert!(stale.is_empty(), "these allowlist entries outlived their files: {stale:?}");
}

struct LoopSite {
    line: usize,
    list: String,
    has_floor: bool,
}

/// Loops over an identity list whose body can `continue`, and whether the enclosing function ever
/// asserts on a count of what the loop examined.
///
/// "A count" is a variable the loop increments (`n += 1`) or extends (`v.push(..)`), named in an
/// `assert` in the same function. That is the shape `spaced_armor.rs` already uses, and it is
/// deliberately the only shape accepted: a floor that is not asserted is a comment.
fn skipping_loops(source: &str) -> Vec<LoopSite> {
    let lines: Vec<&str> = source.lines().collect();
    let mut sites = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let Some(list) = loop_over_identity_list(line) else { continue };
        let Some(end) = block_end(&lines, index) else { continue };
        let body = &lines[index..=end];
        if !body.iter().any(|line| {
            line.trim_start().starts_with("continue")
                || line.contains("else { continue }")
                || line.contains("else { continue };")
        }) {
            continue;
        }
        let counters = counters_in(body);
        let function = function_span(&lines, index);
        // The whole region after the loop, as one string. An `assert!` in this tree routinely puts
        // its condition on the line BELOW the macro name — `spaced_armor.rs` does, and it is the
        // file this rule is modelled on, so a line-at-a-time check declared the exemplar an
        // offender.
        let after_loop = lines[end.min(function.1)..=function.1].join("\n");
        let has_floor = assertions(&after_loop)
            .iter()
            .any(|assertion| counters.iter().any(|counter| names(assertion, counter)));

        sites.push(LoopSite { line: index + 1, list, has_floor });
    }

    sites
}

/// `for <binding> in <Type>::<LIST>` — the list name only, so `T::ALL` and `Other::ALL` read the
/// same. A loop over an arbitrary vector is not a claim about an identity.
fn loop_over_identity_list(line: &str) -> Option<String> {
    let after_in = line.split(" in ").nth(1)?;
    let target = after_in.split_whitespace().next()?.trim_end_matches('{').trim();
    if !line.trim_start().starts_with("for ") {
        return None;
    }
    let list = target.rsplit("::").next()?;
    IDENTITY_LISTS.contains(&list).then(|| target.to_string())
}

/// Index of the line closing the block opened on `start`, by brace depth.
fn block_end(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, line) in lines[start..].iter().enumerate() {
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if depth <= 0 && offset > 0 {
            return Some(start + offset);
        }
    }
    None
}

/// The enclosing function's line range: back to its `fn` line, forward to the column-0 `}`.
fn function_span(lines: &[&str], inside: usize) -> (usize, usize) {
    let start = (0..=inside)
        .rev()
        .find(|index| lines[*index].starts_with("fn ") || lines[*index].starts_with("pub fn "))
        .unwrap_or(0);
    let end = (inside..lines.len()).find(|index| lines[*index] == "}").unwrap_or(lines.len() - 1);
    (start, end)
}

/// Variables the loop body counts into.
fn counters_in(body: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for line in body {
        let trimmed = line.trim();
        if let Some((lhs, _)) = trimmed.split_once(" += 1") {
            names.push(lhs.trim().trim_start_matches("*").to_string());
        }
        if let Some((lhs, _)) = trimmed.split_once(".push(") {
            names.push(lhs.trim().to_string());
        }
        if let Some((lhs, _)) = trimmed.split_once(".insert(") {
            names.push(lhs.trim().to_string());
        }
    }
    names.retain(|name| !name.is_empty() && name.chars().all(|c| c == '_' || c.is_alphanumeric()));
    names.sort();
    names.dedup();
    names
}

/// The full text of each `assert*!(..)` call in a region, parentheses balanced so a multi-line
/// macro is read whole.
fn assertions(region: &str) -> Vec<String> {
    let bytes = region.as_bytes();
    let mut found = Vec::new();
    for (start, _) in region.match_indices("assert") {
        let Some(open) = region[start..].find('(').map(|at| start + at) else { continue };
        let mut depth = 0i32;
        for index in open..bytes.len() {
            match bytes[index] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        found.push(region[start..=index].to_string());
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    found
}

fn names(line: &str, needle: &str) -> bool {
    let bytes = line.as_bytes();
    line.match_indices(needle).any(|(at, _)| {
        let before = at == 0 || !is_word_byte(bytes[at - 1]);
        let after_index = at + needle.len();
        let after = after_index >= bytes.len() || !is_word_byte(bytes[after_index]);
        before && after
    })
}

fn is_word_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}
