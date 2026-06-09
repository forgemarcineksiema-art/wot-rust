use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn rapier_is_not_configured_as_cross_platform_deterministic_gameplay_core() {
    let manifest =
        fs::read_to_string(workspace_root().join("Cargo.toml")).expect("workspace manifest exists");
    let rapier_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("rapier3d ="))
        .expect("workspace declares rapier3d");

    assert!(rapier_line.contains("default-features = false"));
    assert!(rapier_line.contains("\"dim3\""));
    assert!(rapier_line.contains("\"f32\""));
    assert!(!rapier_line.contains("enhanced-determinism"));
    assert!(!rapier_line.contains("parallel"));
    assert!(!rapier_line.contains("simd-stable"));
    assert!(!rapier_line.contains("simd-nightly"));
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("quality crate should live under crates/quality")
        .to_path_buf()
}
