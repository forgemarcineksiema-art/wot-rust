//! Architecture gate: every contract check must be shown to fire.
//!
//! `map_forge::report` is the closest thing this repo has to a specification of what a shippable
//! map is: fifteen named checks, each one a promise that some class of broken map will be refused.
//! A check nobody has ever seen fire is not a promise. It is a branch, and a branch that has never
//! been taken is indistinguishable from a branch that cannot be taken — an inverted comparison, a
//! threshold nothing reaches, a loop that never enters.
//!
//! This is the report half of the same fault `coverage_floor.rs` catches in tests. There, a fleet
//! walk reported on whatever data happened to exist; here, a contract reports on whatever checks
//! happen to fire. Both look like coverage from the outside and neither is.
//!
//! The rule is deliberately crude and therefore hard to satisfy dishonestly: the check's NAME must
//! appear in a test. Naming it means writing a map that trips it, because there is nothing else to
//! assert about a string.

use std::collections::BTreeSet;
use std::fs;

use quality::workspace::{crate_src_dir, integration_test_files, workspace_root};

/// Checks with no test that names them, and why they are not written yet.
///
/// This is the debt, counted. Nine of fifteen when the rule landed: the map contract was built
/// check by check as maps needed them, and the tests were written for the checks whose failures
/// somebody had actually hit. Burn it down by writing the map that trips the check — that map is
/// also the documentation of what the check means.
///
/// **`scenery` came off 2026-08-07 (eight left), and it paid the rule's own thesis back.** Writing
/// the map that trips it found that half of it could not be tripped: the "grows outside the map"
/// warning was unreachable, because both scenery expanders already drop a point the heightmap
/// refuses to ground. That half is deleted and the reachable half — a tree growing through a barn
/// — is now exercised by `scenery_is_refused_when_it_leaves_the_map_or_grows_through_cover`. A
/// branch never taken really was indistinguishable from a branch that cannot be taken.
const UNEXERCISED_CHECK_ALLOWLIST: &[(&str, &str)] = &[
    ("cover_overlap", "no test builds two static cover boxes that interpenetrate"),
    ("environment", "no test ships a map with a malformed environment block"),
    ("grid", "the square-grid refusal is tested; the other grid errors are not named"),
    ("heightmap_sane", "no test feeds the compiler a NaN or runaway height"),
    ("in_bounds", "no test places an object outside the authored rectangle"),
    (
        "materials",
        "the saturation window is tested by name; the rest of the material contract is not",
    ),
    ("roads", "no test authors a road that breaks its own contract"),
    ("water_contract", "the drowning depth is asserted in map tests, never through this check"),
];

#[test]
fn every_contract_check_is_exercised_by_a_test_that_names_it() {
    let root = workspace_root();
    let source = fs::read_to_string(crate_src_dir(&root, "map_forge").join("report.rs"))
        .expect("the map contract report should be readable");

    let tests = exercising_tests(&root);

    let mut unexercised = Vec::new();
    for check in emitted_checks(&source) {
        if tests.contains(&format!("\"{check}\"")) {
            continue;
        }
        if UNEXERCISED_CHECK_ALLOWLIST.iter().any(|(name, _)| *name == check) {
            continue;
        }
        unexercised.push(check);
    }

    assert!(
        unexercised.is_empty(),
        "these contract checks can be emitted but no test has ever seen one fire — a branch never \
         taken is a branch nothing proves is reachable: {unexercised:?}"
    );
}

/// The other direction, and the one that makes the allowlist a ratchet rather than a drawer: an
/// entry that names a check the report no longer emits forgives nothing and hides the next gap.
#[test]
fn the_allowlist_names_only_checks_the_report_still_emits() {
    let root = workspace_root();
    let source = fs::read_to_string(crate_src_dir(&root, "map_forge").join("report.rs"))
        .expect("the map contract report should be readable");
    let emitted = emitted_checks(&source);

    let stale: Vec<&str> = UNEXERCISED_CHECK_ALLOWLIST
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !emitted.contains(*name))
        .collect();

    assert!(
        stale.is_empty(),
        "the allowlist outlived these checks — delete the entries: {stale:?}"
    );
}

/// A check that HAS been exercised must not quietly slide back onto the allowlist: if a test names
/// it, the entry is wrong, and leaving it there would let the check rot again unnoticed.
#[test]
fn no_allowlisted_check_is_actually_exercised() {
    let root = workspace_root();
    let tests = exercising_tests(&root);

    let redundant: Vec<&str> = UNEXERCISED_CHECK_ALLOWLIST
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| tests.contains(&format!("\"{name}\"")))
        .collect();

    assert!(
        redundant.is_empty(),
        "these checks are exercised now — take them off the allowlist so the debt count is true: \
         {redundant:?}"
    );
}

/// Every integration test that could actually trip a contract check — which is every one EXCEPT
/// the gate's own.
///
/// The first draft skipped this and promptly read its own allowlist as proof: the strings live in
/// this file, this file is an integration test, and so all nine debts reported themselves paid. A
/// rule naming a check is not a test tripping it, and a gate that can satisfy itself is worse than
/// no gate — the same self-detection `gate_completeness.rs` hit with `#[ignore]`.
fn exercising_tests(root: &std::path::Path) -> String {
    integration_test_files(root)
        .iter()
        .filter(|path| !path.to_string_lossy().replace('\\', "/").contains("tooling/quality/"))
        .map(|path| fs::read_to_string(path).unwrap_or_default())
        .collect()
}

/// The distinct check names the report can emit, read from its own `push` calls.
fn emitted_checks(source: &str) -> BTreeSet<String> {
    let mut checks = BTreeSet::new();
    let mut rest = source;
    while let Some(at) = rest.find(".push(") {
        rest = &rest[at + ".push(".len()..];
        let head = rest.trim_start();
        let Some(body) = head.strip_prefix('"') else { continue };
        let Some(end) = body.find('"') else { continue };
        let name = &body[..end];
        // `report.push(name, Severity::..)` — a bare string pushed onto some other vector is not a
        // contract check, and the severity argument is what tells them apart.
        if body[end..].contains("Severity")
            && name.chars().all(|c| c == '_' || c == '-' || c.is_ascii_lowercase())
        {
            checks.insert(name.to_string());
        }
    }
    checks
}
