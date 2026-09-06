//! The linker is part of the gate's cost, and the config that picks it is data the gate must see.
//!
//! 2026-09-06 (the build-time program, step 2): MSVC `link.exe` was the bulk of every test build's
//! CPU time; `rust-lld` ships with the toolchain and links the same objects faster. The switch
//! lives in `.cargo/config.toml` at the workspace root, so it applies to every cargo invocation
//! on this machine's target — the gates, `cargo run`, the probes — and never to one of them
//! alone (a linker that differs between the gate and the game is a second kind of ping-pong).

use quality::workspace_root;

#[test]
fn the_workspace_links_with_rust_lld_on_the_msvc_target() {
    let path = workspace_root().join(".cargo/config.toml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is the linker config and must exist", path.display()));
    let section = text
        .split_once("[target.x86_64-pc-windows-msvc]")
        .map(|(_, rest)| rest.split("\n[").next().unwrap_or(rest))
        .expect(".cargo/config.toml has a [target.x86_64-pc-windows-msvc] section");
    assert!(
        section.contains("linker = \"rust-lld.exe\""),
        "the msvc target must name rust-lld.exe as its linker (measured 2026-09-06, see \
         docs/engineering-rules.md § Required Gates):\n{section}"
    );
    assert!(
        section.contains("linker-flavor=lld-link"),
        "rust-lld needs the lld-link flavor on the msvc target:\n{section}"
    );
}
