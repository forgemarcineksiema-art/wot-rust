//! Architecture gate: the merge gate must know which tests it is not running.
//!
//! `scripts/verify.ps1` is the only gate this repo has — CI billing is blocked, so a test that
//! opts itself out of a plain `cargo test` run is a test that never runs at all. That is fine
//! when it is a deliberate, stated decision (a golden that needs a GPU) and dangerous when it is
//! a leftover: the suite reports green either way, and nothing in the output distinguishes
//! "1200 tests passed" from "1200 tests passed and 40 quietly declined".
//!
//! So the rule is not "no opt-outs". It is: every way a test can decline to run is named here,
//! with the reason, where a reader of the gate can count them.

use std::fs;

use quality::workspace::{integration_test_files, repo_relative, workspace_root};

/// Environment variables a test is allowed to consult, with what happens when they are unset.
///
/// `WOT_UPDATE_GOLDENS` is not an opt-OUT — with it unset the test still runs and still compares;
/// setting it re-records instead. It is listed because the scan cannot tell the two apart by
/// reading the source, and because a reader deserves to see both kinds in one place.
const ENV_GATE_ALLOWLIST: &[(&str, &str, &str)] = &[
    (
        "crates/apps/client/tests/look_goldens.rs",
        "WOT_LOOK_GOLDENS",
        "SKIPS by default: the look goldens render real frames through wgpu, so the test needs a \
         GPU that a headless gate run cannot promise. Run it with WOT_LOOK_GOLDENS=1 before any \
         PR that touches lighting, sky, tone mapping or materials.",
    ),
    (
        "crates/apps/client/tests/look_goldens.rs",
        "WOT_UPDATE_GOLDENS",
        "Re-records rather than skips; the default path still asserts.",
    ),
    (
        "crates/apps/tools/tests/studio_goldens.rs",
        "WOT_UPDATE_GOLDENS",
        "Re-records rather than skips; the default path still asserts, and the bless is meant to \
         be read as pictures in target/studio-diff first.",
    ),
    (
        "crates/runtime/net/tests/protocol_snapshots.rs",
        "REGEN_WIRE_FIXTURES",
        "Re-records the wire fixtures rather than skipping. Sharper than the others and worth \
         reading twice: `wire_fixture` writes the file and then reads it back in the same call, so \
         with this set EVERY protocol snapshot passes by construction. Set it for one deliberate \
         protocol bump, then rerun with it UNSET — that second run is the whole test. (One \
         re-record idea, two spellings: WOT_UPDATE_GOLDENS twice above, REGEN_WIRE_FIXTURES here.)",
    ),
];

/// Tests allowed to carry `#[ignore]`, with the reason. Empty, and the tree has none.
const IGNORE_ALLOWLIST: &[(&str, &str)] = &[];

#[test]
fn every_test_that_can_decline_to_run_is_named_here() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for path in integration_test_files(&root) {
        let relative = repo_relative(&path, &root);
        let source = fs::read_to_string(&path).expect("test source should be readable");
        for variable in env_vars_read(&source) {
            let known = ENV_GATE_ALLOWLIST
                .iter()
                .any(|(file, name, _)| *file == relative && *name == variable);
            if !known {
                offenders.push(format!(
                    "{relative} consults `{variable}` — if that can stop the test running, the \
                     gate is reporting green on a test nobody ran. Name it in \
                     ENV_GATE_ALLOWLIST with what happens when it is unset."
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a test the gate silently skips is worse than a missing test, because it looks like \
         coverage:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn no_test_is_ignored_without_a_stated_reason() {
    let root = workspace_root();
    let mut offenders = Vec::new();

    for path in integration_test_files(&root) {
        let relative = repo_relative(&path, &root);
        let source = fs::read_to_string(&path).expect("test source should be readable");
        // An ATTRIBUTE, not the text. The first draft matched the substring anywhere in the file
        // and its first offender was this rule itself, which mentions `#[ignore]` inside a string
        // literal. An attribute starts its line; a mention does not.
        let ignored = source.lines().any(|line| line.trim_start().starts_with("#[ignore"));
        if ignored && !IGNORE_ALLOWLIST.iter().any(|(file, _)| *file == relative) {
            offenders.push(relative);
        }
    }

    assert!(
        offenders.is_empty(),
        "`#[ignore]` hides a test from the only gate this repo has — list it with a reason or \
         delete it:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_allowlists_name_only_files_that_still_exist() {
    let root = workspace_root();
    let present: Vec<String> =
        integration_test_files(&root).iter().map(|path| repo_relative(path, &root)).collect();

    let stale: Vec<&str> = ENV_GATE_ALLOWLIST
        .iter()
        .map(|(file, _, _)| *file)
        .chain(IGNORE_ALLOWLIST.iter().map(|(file, _)| *file))
        .filter(|file| !present.iter().any(|found| found == file))
        .collect();

    assert!(
        stale.is_empty(),
        "an allowlist that outlives its file forgives nothing and hides the next offender: {stale:?}"
    );
}

/// Inny Poziom Q6: there are two gates, and the cheap one is cheap in exactly one place.
///
/// The PR gate (`verify-pr.ps1`) drops the codegen of every example and bench and runs only the
/// touched crates' tests — that is the whole saving, minutes per PR. What it may NOT drop is the
/// whole-workspace, all-targets clippy pass: that is the front-end compile of every probe and
/// bench, and the only reason a PR that breaks a probe's API still fails before it merges. The
/// full gate (`verify.ps1`) keeps the all-targets test run so the day pays what the PR skipped.
/// These are text assertions on scripts because a script's behaviour IS its text; a reader who
/// removes `--all-targets` from the PR gate to save another minute meets this test, not a green
/// merge and a probe that stopped compiling three PRs ago.
#[test]
fn the_pr_gate_fronts_every_target_and_the_full_gate_tests_every_target() {
    let root = workspace_root();
    let pr_gate = fs::read_to_string(root.join("scripts/verify-pr.ps1"))
        .expect("the PR gate script should be readable");
    let full_gate = fs::read_to_string(root.join("scripts/verify.ps1"))
        .expect("the full gate script should be readable");

    const CLIPPY_EVERYWHERE: &str = "cargo clippy --workspace --all-targets -- -D warnings";
    for (name, script) in [("verify-pr.ps1", &pr_gate), ("verify.ps1", &full_gate)] {
        assert!(script.contains("cargo fmt --all -- --check"), "{name} must run rustfmt");
        assert!(
            script.contains(CLIPPY_EVERYWHERE),
            "{name} must front-end every target of every crate: a probe or a bench that stops \
             compiling has to fail here"
        );
    }

    assert!(
        pr_gate.contains("@(\"quality\")") && pr_gate.contains("cargo test --workspace --lib"),
        "the PR gate always runs the ratchet: prepended to whatever crates the PR names, and \
         carried by `--workspace` when it names none"
    );
    assert!(
        !pr_gate.contains("cargo test --workspace --all-targets"),
        "the PR gate's saving is the example and bench codegen it skips; an all-targets test run \
         makes it the full gate under another name"
    );
    assert!(
        full_gate.contains("cargo test --workspace --all-targets"),
        "the full gate tests every target — the day pays what the PR gate skipped"
    );
    assert!(
        full_gate.contains("[switch] $Deep") && full_gate.contains("CARGO_INCREMENTAL"),
        "the full gate keeps a -Deep run that compiles without incremental state (Q5)"
    );
    assert!(
        !root.join("scripts/verify-quick.ps1").exists(),
        "verify-quick.ps1 was retired by the PR gate; a third script is a third truth"
    );
}

/// The names passed to `env::var` / `var_os` in a source file.
fn env_vars_read(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for pattern in ["env::var(\"", "env::var_os(\""] {
        let mut rest = source;
        while let Some(at) = rest.find(pattern) {
            rest = &rest[at + pattern.len()..];
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}
