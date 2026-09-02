//! Architecture gate: the dedicated server and the client compute with the same arithmetic.
//!
//! The predictor's parity locks, the replay-exact fixtures and every "what the sight shows is
//! what the server resolves" promise assume ONE floating-point backend on both ends of the wire.
//! Cargo does not promise that: features are unified per build graph, so a math crate can wear
//! `std` intrinsics in one binary and `libm` in another if any member of one graph asks for a
//! feature the other graph does not (`glam` and `num-traits` both switch their trigonometry on
//! exactly such a feature). Inny Poziom Q5 chased a failing physics lock down this hole and found
//! it was not the cause — the resolutions were identical — but the hazard is real and cheap to
//! lock, so it is locked: `cargo tree` for both binaries, the same feature set for every math
//! crate, and the workspace-wide resolution (the gate's) equal to the client's.

use std::collections::BTreeSet;
use std::process::Command;

use quality::workspace::workspace_root;

/// The crates whose feature set decides which arithmetic runs.
const MATH_CRATES: &[&str] = &["glam", "glamx", "num-traits", "libm", "parry3d"];
/// The crates that must never be absent from either binary's graph — if `cargo tree` finds no
/// such package, the lock would be comparing two empty sets and proving nothing.
const LOAD_BEARING: &[&str] = &["glam", "num-traits"];

/// `cargo tree`'s resolved feature lines for `krate` in the graph selected by `selection`
/// (a `-p <package>` pair or `--workspace`), with the duplicate markers stripped.
fn resolved_features(selection: &[&str], krate: &str) -> BTreeSet<String> {
    // The cargo that compiled this test, baked in at build time: no runtime environment
    // consultation, so the gate cannot silently skip it (`gate_completeness.rs`).
    let cargo = env!("CARGO");
    let mut args: Vec<&str> = vec!["tree", "--offline"];
    args.extend_from_slice(selection);
    args.extend_from_slice(&["-e", "features", "-i", krate, "--prefix", "none"]);
    let output = Command::new(cargo)
        .args(&args)
        .current_dir(workspace_root())
        .output()
        .expect("cargo tree runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        assert!(
            stderr.contains("did not match any packages") || stderr.contains("not found"),
            "cargo tree {}: {stderr}",
            args.join(" ")
        );
        return BTreeSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        // Only the crate's OWN feature lines: the inverted tree also lists every dependent
        // (`client feature "default"`), which differs between graphs by construction.
        .filter(|line| line.starts_with(&format!("{krate} feature \"")))
        .map(|line| {
            line.trim_end_matches(" (*)").trim_end_matches(" (command-line)").trim().to_string()
        })
        .collect()
}

#[test]
fn the_server_and_the_client_resolve_every_math_crate_to_the_same_features() {
    for krate in MATH_CRATES {
        let server = resolved_features(&["-p", "server"], krate);
        let client = resolved_features(&["-p", "client"], krate);
        if LOAD_BEARING.contains(krate) {
            assert!(!server.is_empty() && !client.is_empty(), "{krate}: absent from a binary");
        }
        assert_eq!(
            server, client,
            "{krate}: the server and the client would compute with different arithmetic"
        );
    }
}

/// The gate runs the whole workspace; a developer runs one package. Both must compile the same
/// arithmetic, or a lock can be green at the gate and red at the desk (or the reverse).
#[test]
fn the_workspace_gate_resolves_the_math_crates_exactly_as_the_client_does() {
    for krate in MATH_CRATES {
        let gate = resolved_features(&["--workspace"], krate);
        let client = resolved_features(&["-p", "client"], krate);
        assert_eq!(
            gate, client,
            "{krate}: the workspace gate and a client-only build disagree on features"
        );
    }
}
