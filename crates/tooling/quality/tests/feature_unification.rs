//! Every way the gates and the player invoke cargo resolves every third-party crate to the SAME
//! features.
//!
//! Measured 2026-09-06 on the MX330 laptop: `cargo test --workspace` straight after a per-crate
//! build rebuilt 696 units (3 min 01 s), and `cargo test -p sim` straight after that rebuilt sim
//! again (34 s) with not one source line changed. The resolver unifies features only across the
//! packages named on the command line, so `-p sim` saw `log` without `std`, `smallvec` without
//! `union`, `serde_json` without `raw_value` and `windows-sys` without 37 of its features; every
//! crate above each of them was compiled twice per gate and a third time for `cargo run`. The PR
//! gate's 10–15 minutes were mostly that ping-pong: warm, its real work is ~4 minutes.
//!
//! The cure is `workspace_hack` (`crates/foundation/workspace_hack`): a crate with no code that
//! declares the union of every feature any member ever enables, and that every member depends on,
//! so every selection carries the same graph. This test is what keeps the union complete: a new
//! dependency or feature that reaches one selection and not another fails here, in twenty seconds,
//! not as a mysterious eight-minute rebuild. Regenerate the hack's managed section with
//! `cargo hakari generate` (`cargo install cargo-hakari`; the config is `.config/hakari.toml`),
//! add by hand under `[target.'cfg(all())'.dependencies]` whatever hakari's model leaves out, and
//! re-run this test — it, not `cargo hakari verify`, is the gate.
//!
//! `math_backend_parity.rs` is the older, narrower cousin (server vs client, math crates only);
//! it stays because its failure message names the determinism stake.

use quality::{crate_facts, workspace_root};
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

/// The selections a gate or a player actually runs. Each is one `cargo tree` invocation; the edge
/// kinds say whether dev-dependencies take part (`cargo test` and `clippy --all-targets`: yes;
/// `cargo run`/`cargo build`: no).
const SELECTIONS: &[(&str, &[&str])] = &[
    ("cargo test -p sim", &["-p", "sim", "-e", "normal,build,dev"]),
    ("cargo test -p client", &["-p", "client", "-e", "normal,build,dev"]),
    ("cargo test -p quality -p tools", &["-p", "quality", "-p", "tools", "-e", "normal,build,dev"]),
    ("cargo test -p vehicle_recipes", &["-p", "vehicle_recipes", "-e", "normal,build,dev"]),
    ("cargo test --workspace", &["--workspace", "-e", "normal,build,dev"]),
    ("cargo run -p client", &["-p", "client", "-e", "normal,build"]),
    ("cargo run -p server", &["-p", "server", "-e", "normal,build"]),
    ("cargo run -p editor", &["-p", "editor", "-e", "normal,build"]),
];

/// Third-party feature sets present in one selection's graph, keyed by `name vX.Y.Z`.
///
/// `-f "{p}|{f}"` prints every package once with its resolved features on the same line, so two
/// versions of one crate stay two packages (`windows` 0.54 is cpal's, 0.62 is wgpu's) and no
/// feature-node ordering has to be parsed.
fn resolved_features(
    args: &[&str],
    members: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    // The cargo that compiled this test: no runtime environment consultation, so the gate cannot
    // silently skip it (`gate_completeness.rs`).
    let cargo = env!("CARGO");
    let mut all: Vec<&str> = vec!["tree", "--offline", "--prefix", "none", "-f", "{p}|{f}"];
    all.extend_from_slice(args);
    let output = Command::new(cargo)
        .args(&all)
        .current_dir(workspace_root())
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo {}: {}",
        all.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut features: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim_end_matches(" (*)").trim();
        let Some((package, flags)) = line.split_once('|') else { continue };
        let Some((name, _version)) = package.split_once(' ') else { continue };
        if members.contains(name) {
            continue;
        }
        // A crate absent from a graph is NOT a difference: nothing is built for it, so nothing
        // can be rebuilt (the render surface — wgpu, winit — stays out of the sim and server
        // graphs on purpose, `architecture_rules.rs`). Only a crate present with fewer features
        // than elsewhere costs a rebuild.
        let set = features.entry(package.to_string()).or_default();
        set.extend(flags.split(',').filter(|f| !f.is_empty()).map(str::to_string));
    }
    features
}

#[test]
fn every_selection_resolves_every_third_party_crate_to_the_same_features() {
    let members: BTreeSet<String> =
        crate_facts(&workspace_root()).into_iter().map(|krate| krate.name).collect();
    let graphs: Vec<(&str, BTreeMap<String, BTreeSet<String>>)> = SELECTIONS
        .iter()
        .map(|(label, args)| (*label, resolved_features(args, &members)))
        .collect();

    // The union across every selection is the reference: a selection differs where it lacks a
    // feature (or a whole crate) that another selection carries.
    let mut union: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (_, graph) in &graphs {
        for (krate, features) in graph {
            union.entry(krate.clone()).or_default().extend(features.iter().cloned());
        }
    }
    assert!(union.len() > 100, "the graphs are implausibly small: {} crates", union.len());

    let mut differences = Vec::new();
    for (label, graph) in &graphs {
        for (krate, wanted) in &union {
            let Some(have) = graph.get(krate) else { continue };
            let missing: Vec<_> = wanted.difference(have).cloned().collect();
            if !missing.is_empty() {
                differences.push(format!("{label}: {krate} lacks {}", missing.join(", ")));
            }
        }
    }
    assert!(
        differences.is_empty(),
        "these invocations resolve dependencies differently, so each rebuilds what the other \
         built; add the missing crate/feature to `workspace_hack` (`cargo hakari generate`, then \
         by hand what it leaves out):\n  {}",
        differences.join("\n  ")
    );
}
